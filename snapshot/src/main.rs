use anyhow::Result;
use clap::{Parser, ValueEnum};
use core::num::NonZeroUsize;
use log;
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous, mprotect, munmap};
use rand::prelude::*;
use rand::{Rng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use serde::Serialize;
use std::cmp::{max, min};
use std::fs::File;
use std::io::BufWriter;
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
    Uffd,
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
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
    // Number of times to run the benchmark
    #[arg(short, long, default_value_t = 1)]
    pub runs: u32,
    /// Output file to write timing results to
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(value_enum)]
    pub strategy: DirtyTrackingStrategy,
}

#[derive(Serialize)]
struct ResultStat {
    pub scan: PageMapScanResult,
    pub duration: Duration,
}

fn page_iter(bytes: &mut [u8]) -> impl Iterator<Item = &mut [u8]> {
    bytes.chunks_mut(page_size())
}
/// Touch all the pages in the slice so they are physically allocated
#[allow(dead_code)]
fn touch_pages(bytes: &mut [u8], write: bool) {
    page_iter(bytes).for_each(|s| {
        if write {
            s[0] = 1;
        } else {
            let _acc = s[0] as usize;
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
    for _ in 0..num_ops {
        // Perform a read: calculate a simple checksum
        let sub_slice_range =
            get_random_subrange(slice, &mut read_start, &dist_offset, &dist_len, srng);
        // Touch reads on the pages
        touch_pages(&mut slice[sub_slice_range.clone()], false);

        // Perform a write: Fill with pseudo-random values
        let sub_slice_range =
            get_random_subrange(slice, &mut write_start, &dist_offset, &dist_len, srng);
        log::trace!(
            "Write | {}",
            PageNum::range_string_from_addr(sub_slice_range.start, sub_slice_range.end, 0)?
        );
        // Don't just touch since we want data to be overwritten
        touch_pages(&mut slice[sub_slice_range], true);
    }
    Ok(())
}

/// This method uses the soft-dirty tracking mechanism to perform dirty page tracking
fn soft_dirty_benchmark(
    slice: &mut [u8],
    mut run: impl FnMut(&mut [u8]) -> Result<()>,
    verbose: bool,
) -> Result<ResultStat> {
    // Pagemap scan to see dirty pages
    // PFNZERO is inverted because newly mapped pages have WRITTEN set even after a read.
    // By inverting PFNZERO, we filter out those pages and only get pages that were actually written to afterwards.
    let mut pm_arg = PmScanArgBuilder::new()
        .addr_range_from_slice(slice)
        .category_mask(Categories::SOFT_DIRTY | Categories::WRITTEN | Categories::PFNZERO)
        .category_inverted(Categories::PFNZERO)
        .return_mask(
            Categories::WRITTEN
                | Categories::SOFT_DIRTY
                | Categories::PRESENT
                | Categories::HUGE
                | Categories::FILE
                | Categories::SWAPPED
                | Categories::PFNZERO
                | Categories::WPALLOWED,
        )
        .finish();

    // TBD Weird Bug: Idk what happens with soft dirty here, but if we have a newly mapped
    // soft-dirty pages, it sometimes clears and sometimes it doesn't...
    // It seems to work consistently if we touch the pages first though..
    touch_pages(slice, false);
    clear_soft_dirty_global()?;

    // Run and time the dirty page tracking loop
    run(slice)?;

    let (scan_res, duration) = {
        let start_time = Instant::now();
        let res = pm_arg.run_pagemap_scan()?;
        clear_soft_dirty_global()?;
        (res, start_time.elapsed())
    };

    if verbose {
        log::debug!("Post harness state: {}", scan_res);
    }
    log::debug!(
        "Reset state (post soft-dirty clear): {}",
        pm_arg.run_pagemap_scan()?
    );
    Ok(ResultStat {
        scan: scan_res,
        duration,
    })
}

/// This method uses the written bit in the PTE with Uffd and PAGEMAP_SCAN
fn uffd_benchmark(
    slice: &mut [u8],
    mut run: impl FnMut(&mut [u8]) -> Result<()>,
    verbose: bool,
) -> Result<ResultStat> {
    // Pagemap scan to see dirty pages (similar to `soft_dirty_benchmark`)
    let pm_arg_builder = PmScanArgBuilder::new()
        .addr_range_from_slice(slice)
        .flags(Flags::PM_SCAN_WP_MATCHING | Flags::PM_SCAN_CHECK_WPASYNC)
        .category_mask(Categories::WRITTEN | Categories::PFNZERO)
        .category_inverted(Categories::PFNZERO)
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

    // Run and time the dirty page tracking loop
    run(slice)?;

    let (scan_res, duration) = {
        let start_time = Instant::now();
        let res = pm_arg.run_pagemap_scan()?;
        (res, start_time.elapsed())
    };

    if verbose {
        log::debug!("Post harness state: {}", scan_res);
    }
    // To view the reset state, create a new pm_arg without write protect (just gets the state)
    let mut pm_arg_nowp = pm_arg_builder.clone().flags(Flags::empty()).finish();
    log::debug!(
        "Reset State (post WP clear by scan): {}",
        pm_arg_nowp.run_pagemap_scan()?
    );
    Ok(ResultStat {
        scan: scan_res,
        duration,
    })
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
    let normal = Normal::new(0.0, cli.stddev.unwrap_or((cli.size >> 15) as f64))?;
    log::info!("Using {:?} for random walk", normal);

    // Create a slice from the raw parts
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr.cast::<u8>().as_ptr(), cli.size) };
    // Initialize with random data using a seeded RNG
    let mut srng = StdRng::seed_from_u64(cli.seed);

    let mut run_call = |s: &mut [u8]| run_harness(s, cli.num_ops, normal, &mut srng);
    let results = match cli.strategy {
        DirtyTrackingStrategy::SoftDirty => (0..cli.runs)
            .enumerate()
            .map(|(i, _)| {
                log::info!("Starting Soft-Dirty run {i}...");
                soft_dirty_benchmark(slice, &mut run_call, cli.verbose)
            })
            .collect::<Result<Vec<_>>>()?,

        DirtyTrackingStrategy::Uffd => {
            // UFFD initializiation setup
            let mut uffd = create_uffd(UffdFlags::UFFD_USER_MODE_ONLY)?;
            let api = uffd.api(UffdFeature::WP_ASYNC | UffdFeature::WP_UNPOPULATED)?;
            if !api.ioctls().contains(
                UffdIoctlsSupported::API
                    | UffdIoctlsSupported::REGISTER
                    | UffdIoctlsSupported::UNREGISTER,
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

            // Perform benchmark runs
            (0..cli.runs)
                .enumerate()
                .map(|(i, _)| {
                    log::info!("Starting UFFD run {i}...");
                    uffd_benchmark(slice, &mut run_call, cli.verbose)
                })
                .collect::<Result<Vec<_>>>()?
        }
    };

    if let Some(output_file) = cli.output {
        serde_json::to_writer_pretty(BufWriter::new(File::create(output_file)?), &results)?;
    }
    log::debug!(
        "Scans took {:?}",
        results
            .into_iter()
            .map(|ResultStat { duration, .. }| duration.as_micros())
            .collect::<Vec<_>>()
    );
    log::info!("Completed, cleaning up...");
    // Clean up
    unsafe {
        munmap(ptr, cli.size)?;
    }

    Ok(())
}
