use anyhow::Result;
use clap::Parser;
use log;
use rand::prelude::*;
use rand::{Rng, rngs::StdRng};
use rayon::prelude::*;
use rustix::mm::{MapFlags, ProtFlags, mmap_anonymous, munmap};
use std::ptr;

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

fn get_total_memory() -> usize {
    unsafe {
        let pages = libc::sysconf(libc::_SC_PHYS_PAGES);
        let page_size = libc::sysconf(libc::_SC_PAGESIZE);
        (pages as usize) * (page_size as usize)
    }
}

fn run_memory_hog(alloc_size: usize) -> Result<(*mut libc::c_void, usize)> {
    let total_mem = get_total_memory();
    // Hog a bit more than physical memory available to
    let bytes_to_hog = (total_mem as f64 * 0.98) as usize - alloc_size;
    // Allocate memory
    let ptr = unsafe {
        mmap_anonymous(
            ptr::null_mut(),
            bytes_to_hog,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::PRIVATE,
        )?
    };
    log::info!("Hogging memory...");

    let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, bytes_to_hog) };
    let page_size = 4096;

    // Touch memory to ensure physical allocation
    slice.par_iter_mut().step_by(page_size).for_each(|s| {
        *s = 1;
    });

    Ok((ptr, bytes_to_hog))
}

fn get_random_subslice<'a, R: Rng>(
    src: &'a mut [u8],
    rng: &mut R,
    max_subslice_size: usize,
) -> &'a mut [u8] {
    let size = src.len();
    // Generate random offset and length
    let offset = rng.gen_range(0..size);
    // Determine max possible length to stay within bounds
    let max_len = src.len() - offset;
    // Choose a random length, capped at 64KB for this example to simulate typical chunks,
    // but ensuring we don't exceed the buffer end.
    let len = rng.gen_range(0..=std::cmp::min(max_len, max_subslice_size));
    &mut src[offset..offset + len]
}

/// === Tracking and Snapshotting ===
/// 1. Clear all written/soft dirty bits
/// - Write "4" to /proc/PID/clear_refs
///
/// 2. Perform operations (reads/writes) on the memory region
/// -
///

fn main() -> Result<()> {
    env_logger::init();
    let cli = CLI::parse();

    log::info!("Initializing memory...");
    // mmap the memory
    let ptr = unsafe {
        mmap_anonymous(
            ptr::null_mut(),
            cli.size,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::PRIVATE,
        )?
    };

    log::trace!("Mapped {} bytes at {:p}", cli.size, ptr);

    // Create a slice from the raw parts
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr as *mut u8, cli.size) };

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

    log::info!("Performing {} operations...", cli.num_ops);
    for i in 0..cli.num_ops {
        // Perform a read: calculate a simple checksum
        let sub_slice = get_random_subslice(slice, &mut op_rng, cli.max_slice_size);
        let _sum: u8 = sub_slice.iter().fold(0, |acc, &x| acc.wrapping_add(x));

        // Perform a write: Fill with pseudo-random values
        let sub_slice = get_random_subslice(slice, &mut srng, cli.max_slice_size);
        srng.fill(sub_slice);
        if i % 1000 == 0 {
            log::info!("Completed {i} operations...");
        }
    }

    log::info!("Completed all operations; unmapping");
    // Clean up
    unsafe {
        if let Some((hog_ptr, hog_size)) = hog_mem {
            // Do a random write
            let x = hog_ptr.clone();
            (x as *mut u8).add(35).write(127);
            munmap(hog_ptr, hog_size)?;
        }
        munmap(ptr, cli.size)?;
    }

    Ok(())
}
