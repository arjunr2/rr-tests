use anyhow::Result;
use clap::{Parser, ValueEnum};
use core::num::NonZeroUsize;
use log;
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous, mprotect, munmap};
use rand::prelude::*;
use rand::{Rng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use rayon::prelude::*;
use std::cmp::{max, min};
use std::ops::Range;
use std::time::{Duration, Instant};

mod pagemap;
use pagemap::*;

mod uffd;
use uffd::*;

const MB: usize = 1024 * 1024;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum DirtyTrackingStrategy {
    SoftDirty,
    UffdWritten,
}

#[derive(Parser)]
struct CLI {
    #[arg(short, long, default_value_t = 4096 * MB)]
    pub size: usize,
    #[arg(short, long, default_value_t = 100_000)]
    pub num_ops: u32,
    #[arg(short = 'x', long = "seed", default_value_t = 42)]
    pub seed: u64,
    /// The percentage of total system memory to hog for interference
    #[arg(short, long, default_value_t = false)]
    pub memory_interference: bool,
    /// The stddev of the normal distribution sampled from for random walk
    #[arg(short = 'd', long = "stddev")]
    pub stddev: Option<f64>,
    #[arg(value_enum)]
    pub strategy: DirtyTrackingStrategy,
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
}

/// Touch all the pages in the slice so they are physically allocated
fn touch_pages(bytes: &mut [u8], write: bool) {
    bytes.par_iter_mut().step_by(page_size()).for_each(|s| {
        if write {
            *s = 1;
        } else {
            let _acc = *s as usize;
        };
    });
}

fn get_random_subrange<R: Rng + ?Sized>(
    src: &[u8],
    start: &mut usize,
    dist_offset: &Normal<f64>,
    dist_len: &Normal<f64>,
    rng: &mut R,
) -> Range<usize> {
    // For Uniform:
    // let offset = rng.gen_range(0..size);
    // let len = rng.gen_range(0..=std::cmp::min(max_len, max_subslice_size));
    let size = src.len();
    // Generate random offset and length
    let offset = dist_offset.sample(rng).round() as isize;
    let range_start = ((*start as isize + offset) % (size as isize)) as usize;
    // Determine max possible length to stay within bounds
    let max_len = src.len() - range_start;
    // Choose a length ensuring we don't exceed the buffer end.
    let slen = dist_len.sample(rng).round() as isize;
    let access_len = min(max_len as isize, max(0 as isize, slen)) as usize;
    let ret_range = range_start..range_start + access_len;
    // Set the new start for random walk
    *start = range_start;
    ret_range
}

fn run_harness(
    slice: &mut [u8],
    num_ops: u32,
    dist_offset: Normal<f64>,
    srng: &mut StdRng,
) -> Result<()> {
    // This is the log2 of the length
    let dist_len = Normal::new(2.0, 1.0)?;
    let mut read_start: usize = slice.len() / 2;
    let mut write_start: usize = slice.len() / 2;
    for i in 0..num_ops {
        // Perform a read: calculate a simple checksum
        let sub_slice_range =
            get_random_subrange(slice, &mut read_start, &dist_offset, &dist_len, srng);
        let _sum: u8 = slice[sub_slice_range.clone()]
            .iter()
            .fold(0, |acc, &x| acc.wrapping_add(x));
        log::trace!(
            "Read | {}",
            msg_page_range(sub_slice_range.start, sub_slice_range.end)
        );

        // Perform a write: Fill with pseudo-random values
        let sub_slice_range =
            get_random_subrange(slice, &mut write_start, &dist_offset, &dist_len, srng);
        log::debug!(
            "Write | {}",
            msg_page_range(sub_slice_range.start, sub_slice_range.end)
        );
        srng.fill(&mut slice[sub_slice_range]);
        if i % 1000000 == 0 {
            log::debug!("Completed {i} operations...");
        }
    }
    Ok(())
}

/// This method uses the soft-dirty tracking mechanism to perform dirty page tracking
fn soft_dirty_benchmark(slice: &mut [u8], cli: &CLI, mut srng: StdRng) -> Result<Duration> {
    // Pagemap scan to see written/soft-dirty pages
    let pm_arg_builder = PmScanArgBuilder::new()
        .addr_range_from_slice(slice)
        .category_mask(Categories::SOFT_DIRTY | Categories::WRITTEN)
        .return_mask(
            Categories::WRITTEN
                | Categories::SOFT_DIRTY
                | Categories::PRESENT
                | Categories::HUGE
                | Categories::FILE
                | Categories::SWAPPED
                | Categories::PFNZERO
                | Categories::WPALLOWED,
        );

    let mut pm_arg = pm_arg_builder.clone().finish();

    // TBD Weird Bug: Idk what happens with soft dirty here, but sometimes it clears, sometimes it doesn't
    //log::info!("Initial Scan after mapping: {}", pm_arg.run_pagemap_scan()?);
    touch_pages(slice, false);
    // Initialize by clearing soft dirty
    clear_soft_dirty_global()?;

    run_harness(
        slice,
        cli.num_ops,
        Normal::new(0.0, cli.stddev.unwrap_or((cli.size >> 15) as f64))?,
        &mut srng,
    )?;

    let start_time = Instant::now();
    let scan_res = pm_arg.run_pagemap_scan()?;
    clear_soft_dirty_global()?;
    let duration = start_time.elapsed();
    if cli.verbose {
        log::info!("After harness: {}", scan_res);
    }
    log::info!("After soft-dirty clear: {}", pm_arg.run_pagemap_scan()?);
    Ok(duration)
}

/// This method uses the written bit in the PTE with Uffd and PAGEMAP_SCAN
fn uffd_written_benchmark(slice: &mut [u8], cli: &CLI, mut srng: StdRng) -> Result<Duration> {
    let mut uffd = create_uffd(UffdFlags::UFFD_USER_MODE_ONLY)?;
    let api = uffd.api(UffdFeature::WP_ASYNC | UffdFeature::WP_UNPOPULATED)?;
    if !api.ioctls().contains(
        UffdIoctlsSupported::API | UffdIoctlsSupported::REGISTER | UffdIoctlsSupported::UNREGISTER,
    ) {
        return Err(anyhow::anyhow!(
            "API support incompatible for UFFD: {:?}",
            api
        ));
    }

    let reg = uffd.register(
        slice.as_mut_ptr() as u64,
        slice.len() as u64,
        UffdRegisterMode::MODE_WP,
    )?;
    if !reg.contains(UffdIoctlsSupported::WRITEPROTECT) {
        return Err(anyhow::anyhow!(
            "Write protect support incompatible for UFFD: {:?}",
            reg
        ));
    }
    Ok(Duration::ZERO)
}

fn main() -> Result<()> {
    env_logger::init();
    log::debug!("Page size: {} bytes", page_size());

    let cli = CLI::parse();

    // mmap the memory
    let ptr = unsafe {
        // 8GB Address
        let ptr = mmap_anonymous(
            None,
            NonZeroUsize::new(1usize << 33).unwrap(),
            ProtFlags::PROT_NONE | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )?;
        mprotect(ptr, cli.size, ProtFlags::PROT_READ | ProtFlags::PROT_WRITE)?;
        ptr
    };
    log::info!("Mapped {} bytes at {:p}", cli.size, ptr);
    // Create a slice from the raw parts
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr.cast::<u8>().as_ptr(), cli.size) };
    // Initialize with random data using a seeded RNG
    let srng = StdRng::seed_from_u64(cli.seed);

    let duration = match cli.strategy {
        DirtyTrackingStrategy::SoftDirty => soft_dirty_benchmark(slice, &cli, srng)?,
        DirtyTrackingStrategy::UffdWritten => uffd_written_benchmark(slice, &cli, srng)?,
    };

    log::info!("Scan took {:?}", duration.as_micros());
    log::info!("Completed, cleaning up...");
    // Clean up
    unsafe {
        munmap(ptr, cli.size)?;
    }

    Ok(())
}
