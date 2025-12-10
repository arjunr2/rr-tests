use anyhow::Result;
use nix::sys::mman::{MapFlags, ProtFlags, mmap_anonymous, mprotect, munmap};
use std::num::NonZeroUsize;
use std::ops::Range;
use std::ptr;

pub struct JittedFn {
    ptr: *mut u8,
    size: usize,
}

unsafe impl Send for JittedFn {}
unsafe impl Sync for JittedFn {}

impl JittedFn {
    pub fn new(code: &[u8]) -> Result<Self> {
        let size = NonZeroUsize::new(code.len()).unwrap();
        let ptr = unsafe {
            mmap_anonymous(
                None,
                size,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                MapFlags::MAP_PRIVATE,
            )?
        };

        unsafe {
            ptr::copy_nonoverlapping(code.as_ptr(), ptr.cast().as_ptr(), code.len());
            mprotect(ptr, code.len(), ProtFlags::PROT_READ | ProtFlags::PROT_EXEC)?;
        }

        Ok(Self {
            ptr: ptr.cast().as_ptr(),
            size: code.len(),
        })
    }

    pub fn run(&self, base_ptr: *mut u8, bitmap_ptr: *mut u8) {
        let f: extern "C" fn(*mut u8, *mut u8) = unsafe { std::mem::transmute(self.ptr) };
        f(base_ptr, bitmap_ptr);
    }
}

impl Drop for JittedFn {
    fn drop(&mut self) {
        unsafe {
            if let Some(ptr) = std::ptr::NonNull::new(self.ptr as *mut std::ffi::c_void) {
                let _ = munmap(ptr, self.size);
            }
        }
    }
}

pub struct JitCompiler {
    code: Vec<u8>,
}

impl JitCompiler {
    pub fn new() -> Self {
        Self { code: Vec::new() }
    }

    fn emit(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    fn emit_load_rax_imm64(&mut self, imm: u64) {
        // 48 B8 imm64
        self.emit(&[0x48, 0xB8]);
        self.emit(&imm.to_le_bytes());
    }

    fn emit_load_rsi_imm64(&mut self, imm: u64) {
        // 48 BE imm64
        self.emit(&[0x48, 0xBE]);
        self.emit(&imm.to_le_bytes());
    }

    pub fn compile(
        mut self,
        ranges: &[(Range<usize>, Range<usize>)],
        emulated_bitmap_addr: Option<usize>,
        page_size: usize,
    ) -> Vec<u8> {
        // RDI = base_ptr
        // RSI = bitmap_ptr (loaded from immediate if emulated)

        for (read, write) in ranges {
            // --- READ ---
            self.emit_load_rax_imm64(read.start as u64);
            match read.len() {
                1 => self.emit(&[0x8A, 0x0C, 0x07]),
                2 => self.emit(&[0x66, 0x8B, 0x0C, 0x07]),
                4 => self.emit(&[0x8B, 0x0C, 0x07]),
                8 => self.emit(&[0x48, 0x8B, 0x0C, 0x07]),
                _ => panic!("Unsupported len"),
            }

            //log::trace!(
            //    "Write: {}",
            //    PageNum::range_string_from_addr(write.start, write.end, 0).unwrap()
            //);
            // --- WRITE ---
            self.emit_load_rax_imm64(write.start as u64);
            match write.len() {
                1 => self.emit(&[0xC6, 0x04, 0x07, 0x01]),
                2 => self.emit(&[0x66, 0xC7, 0x04, 0x07, 0x01, 0x00]),
                4 => self.emit(&[0xC7, 0x04, 0x07, 0x01, 0x00, 0x00, 0x00]),
                8 => self.emit(&[0x48, 0xC7, 0x04, 0x07, 0x01, 0x00, 0x00, 0x00]),
                _ => panic!("Unsupported len"),
            }

            // --- EMULATED BITMAP UPDATE ---
            if let Some(addr) = emulated_bitmap_addr {
                let shift = page_size.trailing_zeros() as u8;

                // Update start page
                // RAX contains the offset.
                // Index = RAX >> shift
                // Address = RSI + Index

                // MOV RCX, RAX
                self.emit(&[0x48, 0x89, 0xC1]);
                // SHR RCX, shift
                self.emit(&[0x48, 0xC1, 0xE9, shift]);

                // Load RSI with addr
                self.emit_load_rsi_imm64(addr as u64);

                // MOV BYTE PTR [RSI + RCX], 1
                self.emit(&[0xC6, 0x04, 0x0E, 0x01]);

                // Update end page if different
                // LEA RDX, [RAX + len - 1]
                self.emit(&[0x48, 0x8D, 0x50, (write.len() - 1) as u8]);
                // SHR RDX, shift
                self.emit(&[0x48, 0xC1, 0xEA, shift]);

                // CMP RCX, RDX
                self.emit(&[0x48, 0x39, 0xD1]);
                // JE +4 (skip write)
                self.emit(&[0x74, 0x04]);
                // MOV BYTE PTR [RSI + RDX], 1
                self.emit(&[0xC6, 0x04, 0x16, 0x01]);
            }
        }

        self.emit(&[0xC3]); // RET
        self.code
    }
}
