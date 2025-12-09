//! A simple wrapper around the pagemap scanning functionality for snapshotting

use anyhow::{Result, ensure};
use core::ops::Range;
use core::{fmt, mem};
use procfs::process::{ClearRefs, Process};
use serde::Serialize;
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::thread;
use std::time::Duration;

pub fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

#[allow(dead_code)]
pub fn get_total_memory() -> usize {
    unsafe {
        let pages = libc::sysconf(libc::_SC_PHYS_PAGES);
        (pages as usize) * page_size()
    }
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct PageNum(usize);

impl PageNum {
    pub fn from_addr(addr: usize) -> Self {
        let page_bytes = page_size().ilog2();
        //if align_down {
        //    if addr & (page_size() - 1) == 0 {
        //        addr -= page_size();
        //    }
        //}
        Self(addr >> page_bytes)
    }

    pub fn from_addr_relative(addr: usize, relative_start: usize) -> Result<Self> {
        ensure!(
            relative_start <= addr,
            "Relative start address {:#x} is greater than address {:#x} for pagenum",
            relative_start,
            addr
        );
        Ok(Self::from_addr(addr - relative_start))
    }

    pub fn range_string_from_pagenums(
        start: PageNum,
        mut end: PageNum,
        inclusive_end: bool,
    ) -> Result<String> {
        if !inclusive_end {
            end.0 -= 1;
        }
        Ok(format!(
            "Pages: {}",
            if start.0 > end.0 {
                "NIL".to_string()
            } else if start.0 == end.0 {
                format!("{:#x}", start.0)
            } else {
                format!("{:#x} ... {:#x}", start.0, end.0)
            },
        ))
    }

    pub fn range_string_from_addr(
        start: usize,
        end: usize,
        relative_start: usize,
    ) -> Result<String> {
        let start_page = Self::from_addr_relative(start, relative_start)?;
        let end_page = Self::from_addr_relative(end, relative_start)?;
        Self::range_string_from_pagenums(start_page, end_page, true)
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
pub struct PageRegionRaw {
    pub start: u64,
    pub end: u64,
    pub categories: Categories,
}

#[derive(Default, Debug, Clone)]
struct PageMapScanResultRaw {
    start_addr: u64,
    regions: Vec<PageRegionRaw>,
    walk_end: u64,
}

#[derive(Debug, Serialize)]
pub struct PageRegion {
    pub start: PageNum,
    pub end: PageNum,
    #[serde(skip)]
    pub categories: Categories,
}

impl PageRegion {
    pub fn from_raw(raw: PageRegionRaw, relative_start: usize) -> Result<Self> {
        Ok(Self {
            start: PageNum::from_addr_relative(raw.start as usize, relative_start)?,
            end: PageNum::from_addr_relative(raw.end as usize, relative_start)?,
            categories: raw.categories,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct PageMapScanResult {
    walk_start: PageNum,
    walk_end: PageNum,
    regions: Vec<PageRegion>,
}

impl PageMapScanResult {
    fn from_raw(raw: PageMapScanResultRaw, relative_start: usize) -> Result<Self> {
        let walk_start = PageNum::from_addr_relative(raw.start_addr as usize, relative_start)?;
        let walk_end = PageNum::from_addr_relative(raw.walk_end as usize, relative_start)?;
        Ok(Self {
            walk_start,
            walk_end,
            regions: raw
                .regions
                .into_iter()
                .map(|r| PageRegion::from_raw(r, relative_start))
                .collect::<Result<Vec<PageRegion>>>()?,
        })
    }
}

impl fmt::Display for PageMapScanResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "==== PAGEMAP SCAN (Scan ended before Page {:#x}) ====",
            self.walk_start.0
        )?;
        for region in &self.regions {
            writeln!(
                f,
                "{} | {:?}",
                PageNum::range_string_from_pagenums(region.start, region.end, false).unwrap(),
                region.categories
            )?;
        }
        writeln!(f, "========================================")?;
        Ok(())
    }
}

impl PmScanArg {
    pub fn run_pagemap_scan(&mut self) -> Result<PageMapScanResult> {
        let mut pm_res = PageMapScanResultRaw::default();
        pm_res.start_addr = self.start;

        // If max_pages is 0, set output vector size to max of the length
        let vec_cap = if self.max_pages == 0 {
            ((self.end - self.start) as usize / page_size()) + 1
        } else {
            self.max_pages as usize
        };
        // The ioctl returns the length of the output vector, but we need to conservatively allocate
        pm_res.regions = vec![PageRegionRaw::default(); vec_cap];

        // Set the pointers for output
        self.vec = pm_res.regions.as_mut_ptr() as u64;
        self.vec_len = vec_cap as u64;

        // Generate the ioctl wrapper and run
        let pagemap = File::open("/proc/self/pagemap")?;
        nix::ioctl_readwrite!(pm_scan_ioctl_cmd, b'f', 16, PmScanArg);
        let result = unsafe { pm_scan_ioctl_cmd(pagemap.as_raw_fd(), self as *mut PmScanArg)? };
        if result < 0 {
            return Err(io::Error::last_os_error().into());
        }

        // Update output walk end
        pm_res.walk_end = self.walk_end;
        // Number of elements in the vector is the result
        unsafe {
            pm_res.regions.set_len(result as usize);
        }

        Ok(PageMapScanResult::from_raw(pm_res, self.start as usize)?)
    }
}

#[derive(Clone)]
pub struct PmScanArgBuilder(PmScanArg);

impl PmScanArgBuilder {
    pub fn new() -> Self {
        Self(PmScanArg::default())
    }

    /// Set the address range to scan. The end address is exclusive for the pagemap_scan
    #[allow(dead_code)]
    pub fn addr_range(mut self, range: Range<u64>) -> Self {
        self.0.start = range.start;
        self.0.end = range.end;
        self
    }

    pub fn addr_range_from_slice(mut self, slice: &[u8]) -> Self {
        let start = slice.as_ptr();
        self.0.start = start as u64;
        self.0.end = unsafe { start.offset(slice.len() as isize) as u64 };
        self
    }

    #[allow(dead_code)]
    pub fn flags(mut self, flags: Flags) -> Self {
        self.0.flags = flags;
        self
    }

    pub fn finish(self) -> PmScanArg {
        self.0
    }

    /// 0 means no limit
    #[allow(dead_code)]
    pub fn max_pages(mut self, max_pages: u64) -> Self {
        self.0.max_pages = max_pages;
        self
    }

    #[allow(dead_code)]
    pub fn category_inverted(mut self, category_inverted: Categories) -> Self {
        self.0.category_inverted = category_inverted;
        self
    }

    #[allow(dead_code)]
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

/// This doesn't clear WRITTEN bit; only SOFT_DIRTY. Need mprotect for that.
pub fn clear_soft_dirty_global() -> Result<()> {
    log::trace!("Clearing soft-dirty bits globally...");
    println!("ID: {}", Process::myself()?.pid);
    Process::myself()?.clear_refs(ClearRefs::SoftDirty)?;
    // Sleep a bit to ensure the clear takes effect before we proceed
    thread::sleep(Duration::from_secs(10));
    Ok(())
}
