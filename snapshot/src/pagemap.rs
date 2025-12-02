//! A simple wrapper around the pagemap scanning functionality for snapshotting

use anyhow::Result;
use core::mem;
use core::ops::Range;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsRawFd;

const PAGEMAP_PATH: &str = "/proc/self/pagemap";
const CLEARREFS_PATH: &str = "/proc/self/clear_refs";

pub fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

pub fn get_total_memory() -> usize {
    unsafe {
        let pages = libc::sysconf(libc::_SC_PHYS_PAGES);
        (pages as usize) * page_size()
    }
}

/// Input `struct pm_scan_arg` for pagemap scan ioctl
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct PmScanArg {
    size: u64,
    flags: Flags,
    start: u64,
    end: u64,
    walk_end: u64,
    vec: u64,
    vec_len: u64,
    max_pages: u64,
    category_inverted: Categories,
    category_mask: Categories,
    category_anyof_mask: Categories,
    return_mask: Categories,
}

bitflags::bitflags! {
    /// Categories that can be filtered with [`PageMapScan`]
    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct Categories: u64 {
        /// The page has asynchronous write-protection enabled.
        const WPALLOWED = 1 << 0;
        /// The page has been written to from the time it was write protected.
        const WRITTEN = 1 << 1;
        /// The page is file backed.
        const FILE = 1 << 2;
        /// The page is present in the memory.
        const PRESENT = 1 << 3;
        /// The page is swapped.
        const SWAPPED = 1 << 4;
        /// The page has zero PFN.
        const PFNZERO = 1 << 5;
        /// The page is THP or Hugetlb backed.
        const HUGE = 1 << 6;
        /// The page soft-dirty bit is set.
        const SOFT_DIRTY = 1 << 7;
    }
}

bitflags::bitflags! {
    /// Categories that can be filtered with [`PageMapScan`]
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct Flags: u64 {
        /// Write protect the matched pages
        const PM_SCAN_WP_MATCHING = 1 << 0;
        /// Abort the scan when a page without Userfaultfd Asynchronous Write Protection is encountered
        const PM_SCAN_CHECK_WPASYNC = 1 << 1;
    }
}

impl Default for PmScanArg {
    fn default() -> Self {
        PmScanArg {
            size: mem::size_of::<PmScanArg>() as u64,
            flags: Flags::empty(),
            start: 0,
            end: 0,
            walk_end: 0,
            vec: 0,
            vec_len: 0,
            max_pages: 0,
            category_inverted: Categories::empty(),
            category_mask: Categories::empty(),
            category_anyof_mask: Categories::empty(),
            return_mask: Categories::empty(),
        }
    }
}

/// Output `page_region` from pagemap scan
#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct PageRegion {
    pub start: u64,
    pub end: u64,
    pub categories: Categories,
}

#[derive(Default, Debug, Clone)]
pub struct PageMapScanResult {
    pub regions: Vec<PageRegion>,
    pub walk_end: u64,
}

//#[allow(dead_code)]
//pub fn walk_end(mut self, walk_end: &'a mut u64) -> Self {
impl PmScanArg {
    pub fn run_pagemap_scan(&mut self) -> Result<PageMapScanResult> {
        let mut res = PageMapScanResult::default();

        // If max_pages is 0, set output vector size to max of the length
        let vec_cap = if self.max_pages == 0 {
            ((self.end - self.start) as usize / page_size()) + 1
        } else {
            self.max_pages as usize
        };
        res.regions = Vec::with_capacity(vec_cap);

        // Set the pointers for output
        self.vec = res.regions.as_mut_ptr() as u64;
        self.walk_end = &res.walk_end as *const u64 as u64;

        let pagemap = File::open(PAGEMAP_PATH)?;
        // Generate the ioctl wrapper
        nix::ioctl_readwrite!(pm_scan_ioctl_cmd, b'f', 16, PmScanArg);

        println!("Before ioctl: {:?}", self);
        let result = unsafe { pm_scan_ioctl_cmd(pagemap.as_raw_fd(), self as *mut PmScanArg)? };
        // Update output vector length
        println!("After ioctl: {:?}", self);
        unsafe {
            res.regions.set_len(self.vec_len as usize);
        }
        if result < 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(res)
    }
}

pub struct PmScanArgBuilder(PmScanArg);

impl PmScanArgBuilder {
    pub fn new() -> Self {
        Self(PmScanArg::default())
    }

    /// Set the address range to scan. The end address is exclusive for the pagemap_scan
    pub fn addr_range(mut self, range: Range<u64>) -> Self {
        self.0.start = range.start;
        self.0.end = range.end;
        self
    }

    pub fn flags(mut self, flags: Flags) -> Self {
        self.0.flags = flags;
        self
    }

    pub fn finish(self) -> PmScanArg {
        self.0
    }

    /// 0 means no limit
    pub fn max_pages(mut self, max_pages: u64) -> Self {
        self.0.max_pages = max_pages;
        self
    }

    #[allow(dead_code)]
    pub fn category_inverted(mut self, category_inverted: Categories) -> Self {
        self.0.category_inverted = category_inverted;
        self
    }

    pub fn category_mask(mut self, category_mask: Categories) -> Self {
        self.0.category_mask = category_mask;
        self
    }

    #[allow(dead_code)]
    pub fn category_anyof_mask(mut self, category_anyof_mask: Categories) -> Self {
        self.0.category_anyof_mask = category_anyof_mask;
        self
    }

    pub fn return_mask(mut self, return_mask: Categories) -> Self {
        self.0.return_mask = return_mask;
        self
    }
}

pub fn clear_refs() -> Result<()> {
    let mut file = File::options().write(true).open(CLEARREFS_PATH)?;
    file.write_all(b"4")?;
    Ok(())
}

pub fn print_pagemap_results(vec: &[PageRegion], start: u64, end: u64, walk_end: u64) {
    log::info!("========= Pagemap Scan Results ========= ");
    log::info!(
        "Scanned range: {:#x} -- {:#x} (end: {:#x})",
        start,
        walk_end,
        end
    );
    for region in vec {
        log::info!(
            "Region: {:#x} - {:#x}, Categories: {:?}",
            region.start,
            region.end,
            region.categories
        );
    }
}
