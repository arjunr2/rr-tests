use anyhow::Result;
use clap::Parser;
use core::num::NonZeroUsize;
use log;
use nix::sys::mman::{mmap_anonymous, munmap, MapFlags, ProtFlags};
use rand::prelude::*;
use rand::{rngs::StdRng, Rng};
use rayon::prelude::*;
use std::ops::Range;
use std::ptr::NonNull;

mod pagemap;
use pagemap::*;

const MB: usize = 1024 * 1024;
const KB: usize = 1024;

#[derive(Parser)]
struct CLI {
    #[arg(short, long, default_value_t = 4096 * MB)]
    pub size: usize,
    #[arg(short, long, default_value_t = 100_000)]
    pub num_ops: u32,
    #[arg(short = 'l', long = "slice-size", default_value_t = 4 * KB)]
    pub max_slice_size: usize,
    /// The percentage of total system memory to hog for interference
    #[arg(short, long, default_value_t = false)]
    pub memory_interference: bool,
}

fn run_memory_hog(alloc_size: usize) -> Result<(NonNull<libc::c_void>, usize)> {
    let total_mem = get_total_memory();
    // Hog a bit more than physical memory available to
    let bytes_to_hog = (total_mem as f64 * 0.98) as usize - alloc_size;
    // Allocate memory
    let ptr = unsafe {
        mmap_anonymous(
            None,
            NonZeroUsize::new(bytes_to_hog).unwrap(),
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )?
    };
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr.cast::<u8>().as_ptr(), bytes_to_hog) };

    log::info!("Hogging memory...");

    // Touch memory to ensure physical allocation
    slice.par_iter_mut().step_by(page_size()).for_each(|s| {
        *s = 1;
    });

    Ok((ptr, bytes_to_hog))
}

fn get_random_subrange<R: Rng>(src: &[u8], rng: &mut R, max_subslice_size: usize) -> Range<usize> {
    let size = src.len();
    // Generate random offset and length
    let offset = rng.gen_range(0..size);
    // Determine max possible length to stay within bounds
    let max_len = src.len() - offset;
    // Choose a random length, capped at 64KB for this example to simulate typical chunks,
    // but ensuring we don't exceed the buffer end.
    let len = rng.gen_range(0..=std::cmp::min(max_len, max_subslice_size));
    offset..offset + len
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = CLI::parse();

    log::info!("Initializing memory...");
    // mmap the memory
    let ptr = unsafe {
        mmap_anonymous(
            None,
            NonZeroUsize::new(cli.size).unwrap(),
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_PRIVATE,
        )?
    };

    log::debug!("Mapped {} bytes at {:p}", cli.size, ptr);

    // Create a slice from the raw parts
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr.cast::<u8>().as_ptr(), cli.size) };

    // Initialize with random data using a seeded RNG
    let mut srng = StdRng::seed_from_u64(42);
    // True random RNG
    let mut op_rng = thread_rng();

    let hog_mem = if cli.memory_interference {
        // We just need this to stay for interference
        Some(run_memory_hog(cli.size)?)
    } else {
        None
    };

    log::info!(
        "Clearing soft dirty and performing {} operations...",
        cli.num_ops
    );
    clear_refs()?;
    for i in 0..cli.num_ops {
        // Perform a read: calculate a simple checksum
        let sub_slice_range = get_random_subrange(slice, &mut op_rng, cli.max_slice_size);
        log::trace!(
            "Read range: {:x} -- {:x}",
            sub_slice_range.start,
            sub_slice_range.end
        );
        let _sum: u8 = slice[sub_slice_range]
            .iter()
            .fold(0, |acc, &x| acc.wrapping_add(x));

        // Perform a write: Fill with pseudo-random values
        let sub_slice_range = get_random_subrange(slice, &mut srng, cli.max_slice_size);
        log::debug!(
            "Write range: {:x} -- {:x}",
            sub_slice_range.start,
            sub_slice_range.end
        );
        srng.fill(&mut slice[sub_slice_range]);
        if i % 1000 == 0 {
            log::info!("Completed {i} operations...");
        }
    }

    // Pagemap scan to see written/soft-dirty pages
    let start_ptr = slice.as_ptr() as u64;
    let end_ptr = start_ptr + slice.len() as u64;
    let max_pages = cli.size / page_size();
    let mut vec: Vec<PageRegion> = Vec::with_capacity(max_pages);
    let mut walk_end: u64 = 0;
    let mut pm_arg = PmScanArgBuilder::new()
        .addr_range(start_ptr..end_ptr)
        .max_pages(max_pages as u64)
        .category_mask(Categories::WRITTEN)
        .return_mask(
            Categories::WRITTEN | Categories::SOFT_DIRTY | Categories::PRESENT | Categories::HUGE,
        )
        .finish();

    let res = pm_arg.run_pagemap_scan()?;
    log::info!("{:?}", res);

    log::info!("Completed all operations; unmapping");
    // Clean up
    unsafe {
        if let Some((hog_ptr, hog_size)) = hog_mem {
            // Do a random write
            let x = hog_ptr.clone();
            x.cast::<u8>().add(35).write(127);
            munmap(hog_ptr, hog_size)?;
        }
        munmap(ptr, cli.size)?;
    }

    Ok(())
}
