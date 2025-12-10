use anyhow::Result;
use clap::{Parser, ValueEnum};
use core::num::NonZeroUsize;
use log;
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous, mprotect, munmap};
use rand::prelude::*;
use rand::{Rng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use serde::Serialize;
use std::cmp::min;
use std::fs::File;
use std::io::BufWriter;
use std::ops::Range;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

mod pagemap;
use pagemap::*;

mod uffd;
use uffd::*;

mod jit;
use jit::{JitCompiler, JittedFn};

const MB: usize = 1024 * 1024;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum, Serialize)]
enum DirtyTrackingStrategy {
    SoftDirty,
    Uffd,
    EmulatedSoftDirty,
}

#[derive(Parser)]
struct CLI {
    #[arg(short, long, default_value_t = 4096 * MB)]
    pub size: usize,
    #[arg(short, long, default_value_t = 100_000)]
    pub num_ops: u32,
    #[arg(short = 'x', long = "seed", default_value_t = 42)]
    pub seed: u64,
    /// The stddev of the normal distribution sampled from for random walk
    #[arg(short = 'd', long = "stddev")]
    pub stddev: Option<f64>,
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,
    /// Number of times to run the benchmark
    #[arg(short, long, default_value_t = 1)]
    pub runs: u32,
    /// Warmup runs
    ///
    /// These do not show up in the final stats
    #[arg(short, long, default_value_t = 3)]
    pub warmup_runs: u32,
    /// Output file to write timing results to
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(value_enum)]
    pub strategy: DirtyTrackingStrategy,
}

struct SoftDirtyBitmap(pub Vec<u8>);

#[derive(Clone, Serialize)]
struct ResultStat {
    pub scan: PageMapScanResult,
    pub scan_duration: Duration,
    pub harness_duration: Duration,
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
    // Choose a length ensuring we don't exceed the buffer end, and it's
    let slen = dist_len.sample(rng).round().min(3.0).max(0.0) as isize;
    // Access len is now [1, 2, 4, 8] bytes
    let access_len = 1 << min(max_len as isize, slen) as usize;
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
) -> Result<Duration> {
    // This is the log2 of the length
    let dist_len = Normal::new(1.5, 1.0)?;
    let mut read_start: usize = slice.len() / 2;
    let mut write_start: usize = slice.len() / 2;

    let read_write_ranges = (0..num_ops)
        .map(|_| {
            (
                get_random_subrange(slice, &mut read_start, &dist_offset, &dist_len, srng),
                get_random_subrange(slice, &mut write_start, &dist_offset, &dist_len, srng),
            )
        })
        .collect::<Vec<_>>();

    let compiler = JitCompiler::new();
    let code = compiler.compile(&read_write_ranges, None, page_size());
    let jit_fn = JittedFn::new(&code)?;

    let start_time = Instant::now();
    jit_fn.run(slice.as_mut_ptr(), std::ptr::null_mut());
    Ok(start_time.elapsed())
}

fn run_harness_emulated_dirty(
    slice: &mut [u8],
    num_ops: u32,
    dist_offset: Normal<f64>,
    srng: &mut StdRng,
) -> Result<(Duration, SoftDirtyBitmap)> {
    // This is the log2 of the length
    let dist_len = Normal::new(1.5, 1.0)?;
    let mut read_start: usize = slice.len() / 2;
    let mut write_start: usize = slice.len() / 2;
    let mut bitmap_store =
        SoftDirtyBitmap(vec![0u8; (slice.len() + page_size() - 1) / page_size()]);
    let bitmap = &mut bitmap_store.0[..];

    let read_write_ranges = (0..num_ops)
        .map(|_| {
            (
                get_random_subrange(slice, &mut read_start, &dist_offset, &dist_len, srng),
                get_random_subrange(slice, &mut write_start, &dist_offset, &dist_len, srng),
            )
        })
        .collect::<Vec<_>>();

    let compiler = JitCompiler::new();
    let code = compiler.compile(
        &read_write_ranges,
        Some(bitmap.as_mut_ptr() as usize),
        page_size(),
    );
    let jit_fn = JittedFn::new(&code)?;

    let start_time = Instant::now();
    jit_fn.run(slice.as_mut_ptr(), bitmap.as_mut_ptr());
    let duration = start_time.elapsed();
    Ok((duration, bitmap_store))
}

/// This method uses the soft-dirty tracking mechanism to perform dirty page tracking
fn soft_dirty_benchmark(
    slice: &mut [u8],
    mut run: impl FnMut(&mut [u8]) -> Result<Duration>,
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
    // It seems to work more consistently if we touch the pages first though..
    touch_pages(slice, false);
    clear_soft_dirty_global()?;

    // Run and time the dirty page tracking loop
    let harness_duration = run(slice)?;

    let (scan_res, duration) = {
        let start_time = Instant::now();
        let res = pm_arg.run_pagemap_scan_till_end()?;
        clear_soft_dirty_global()?;
        (res, start_time.elapsed())
    };

    if verbose {
        log::debug!("Post harness state: {}", scan_res);
    }
    let reset_state = pm_arg.run_pagemap_scan_till_end()?;
    assert!(
        reset_state.is_regions_empty(),
        "Expected empty reset state after soft-dirty clear: {}",
        reset_state
    );
    Ok(ResultStat {
        scan: scan_res,
        scan_duration: duration,
        harness_duration,
    })
}

/// This method uses the written bit in the PTE with Uffd and PAGEMAP_SCAN
fn uffd_benchmark(
    slice: &mut [u8],
    mut run: impl FnMut(&mut [u8]) -> Result<Duration>,
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
    let harness_duration = run(slice)?;

    let (scan_res, duration) = {
        let start_time = Instant::now();
        let res = pm_arg.run_pagemap_scan_till_end()?;
        (res, start_time.elapsed())
    };

    if verbose {
        log::debug!("Post harness state: {}", scan_res);
    }
    // To view the reset state, create a new pm_arg without write protect (just gets the state)
    let mut pm_arg_nowp = pm_arg_builder.clone().flags(Flags::empty()).finish();
    let reset_state = pm_arg_nowp.run_pagemap_scan_till_end()?;
    assert!(
        reset_state.is_regions_empty(),
        "Expected empty reset state after WP clear: {}",
        reset_state
    );
    Ok(ResultStat {
        scan: scan_res,
        scan_duration: duration,
        harness_duration,
    })
}

fn main() -> Result<()> {
    env_logger::init();
    log::debug!("Page size: {} bytes", page_size());

    let cli = CLI::parse();

    // Open the pagemap and clear_refs files
    LazyLock::force(&PAGEMAP_FILE);
    LazyLock::force(&CLEAR_REFS_FILE);

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
    let total_runs = cli.runs + cli.warmup_runs;
    let results = match cli.strategy {
        DirtyTrackingStrategy::EmulatedSoftDirty => (0..total_runs)
            .enumerate()
            .map(|(i, _)| {
                log::info!("Starting Emulated Soft-Dirty run {i}...");
                let (harness_duration, bitmap) =
                    run_harness_emulated_dirty(slice, cli.num_ops, normal, &mut srng)?;
                Ok(ResultStat {
                    scan: PageMapScanResult::from_bitmap(bitmap, Categories::empty()),
                    scan_duration: Duration::ZERO,
                    harness_duration,
                })
            })
            .collect::<Result<Vec<_>>>()?,

        DirtyTrackingStrategy::SoftDirty => (0..total_runs)
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
            (0..total_runs)
                .enumerate()
                .map(|(i, _)| {
                    log::info!("Starting UFFD run {i}...");
                    uffd_benchmark(slice, &mut run_call, cli.verbose)
                })
                .collect::<Result<Vec<_>>>()?
        }
    };

    if let Some(output_file) = cli.output {
        #[derive(Serialize)]
        struct BenchmarkOutput {
            strategy: DirtyTrackingStrategy,
            results: Vec<ResultStat>,
        }

        let output = BenchmarkOutput {
            strategy: cli.strategy,
            results: results[cli.warmup_runs as usize..].to_vec(),
        };
        serde_json::to_writer_pretty(BufWriter::new(File::create(output_file)?), &output)?;
    }
    log::info!("Completed, cleaning up...");
    // Clean up
    unsafe {
        munmap(ptr, cli.size)?;
    }

    Ok(())
}
