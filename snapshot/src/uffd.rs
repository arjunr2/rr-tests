use anyhow::Result;
use libc::{SYS_userfaultfd, syscall};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

impl AsRawFd for Uffd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl FromRawFd for Uffd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Uffd(fd)
    }
}

impl IntoRawFd for Uffd {
    fn into_raw_fd(self) -> RawFd {
        self.0
    }
}

bitflags::bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct UffdFlags: u32 {
        const O_CLOEXEC = libc::O_CLOEXEC as u32;
        const O_NONBLOCK = libc::O_NONBLOCK as u32;
        const UFFD_USER_MODE_ONLY = 1;
    }
}

bitflags::bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct UffdRegisterMode: u64 {
        const MODE_MISSING = 1 << 0;
        const MODE_WP = 1 << 1;
        const MODE_MINOR = 1 << 2;
    }
}

pub mod uffd_consts {
    // For capabilities
    pub const _UFFDIO_REGISTER: u64 = 0x00;
    pub const _UFFDIO_UNREGISTER: u64 = 0x01;
    pub const _UFFDIO_WAKE: u64 = 0x02;
    pub const _UFFDIO_COPY: u64 = 0x03;
    pub const _UFFDIO_ZEROPAGE: u64 = 0x04;
    pub const _UFFDIO_MOVE: u64 = 0x05;
    pub const _UFFDIO_WRITEPROTECT: u64 = 0x06;
    pub const _UFFDIO_CONTINUE: u64 = 0x07;
    pub const _UFFDIO_POISON: u64 = 0x08;
    pub const _UFFDIO_API: u64 = 0x3F;

    // For IO
    pub const UFFDIO: u8 = 0xAA;

    // For API version
    pub const UFFD_API: u64 = 0xAA;
}

bitflags::bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct UffdIoctlsSupported: u64 {
        // For UFFDIO_API ioctl
        const API = 1 << uffd_consts::_UFFDIO_API;
        const REGISTER = 1 << uffd_consts::_UFFDIO_REGISTER;
        const UNREGISTER =  1 << uffd_consts::_UFFDIO_UNREGISTER;
        // For UFFDIO_REGISTER ioctl
        const COPY = 1 << uffd_consts::_UFFDIO_COPY;
        const WAKE = 1 << uffd_consts::_UFFDIO_WAKE;
        const WRITEPROTECT = 1 << uffd_consts::_UFFDIO_WRITEPROTECT;
        const ZEROPAGE = 1 << uffd_consts::_UFFDIO_ZEROPAGE;
        const POISON = 1 << uffd_consts::_UFFDIO_POISON;
        const CONTINUE = 1 << uffd_consts::_UFFDIO_CONTINUE;
    }
}

bitflags::bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct UffdFeature: u64 {
        const PAGEFAULT_FLAG_WP = 1 << 0;
        const EVENT_FORK = 1 << 1;
        const EVENT_REMAP = 1 << 2;
        const EVENT_REMOVE = 1 << 3;
        const MISSING_HUGETLBFS = 1 << 4;
        const MISSING_SHMEM = 1 << 5;
        const EVENT_UNMAP = 1 << 6;
        const SIGBUS = 1 << 7;
        const THREAD_ID = 1 << 8;
        const MINOR_HUGETLBFS = 1 << 9;
        const MINOR_SHMEM = 1 << 10;
        const EXACT_ADDRESS = 1 << 11;
        const WP_HUGETLBFS_SHMEM = 1 << 12;
        const WP_UNPOPULATED = 1 << 13;
        const POISON = 1 << 14;
        const WP_ASYNC = 1 << 15;
        const MOVE = 1 << 16;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct UffdApi {
    api: u64,
    features: UffdFeature,
    ioctls: UffdIoctlsSupported,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct UffdRange {
    start: u64,
    len: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
struct UffdioRegister {
    range_start: UffdRange,
    mode: UffdRegisterMode,
    ioctls: UffdIoctlsSupported,
}

impl UffdApi {
    pub fn ioctls(&self) -> UffdIoctlsSupported {
        self.ioctls
    }
}

pub struct Uffd(RawFd);

impl Uffd {
    pub fn api(&mut self, features: UffdFeature) -> Result<UffdApi> {
        let mut api = UffdApi {
            api: uffd_consts::UFFD_API,
            features,
            ioctls: UffdIoctlsSupported::default(),
        };

        // Generate the ioctl wrapper
        nix::ioctl_readwrite!(
            uffd_api_ioctl_cmd,
            uffd_consts::UFFDIO,
            uffd_consts::_UFFDIO_API as u8,
            UffdApi
        );

        let result = unsafe { uffd_api_ioctl_cmd(self.as_raw_fd(), &mut api as *mut UffdApi)? };
        if result < 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(api)
    }

    // Register a memory range with the userfaultfd, and return the supported ioctls
    pub fn register(
        &mut self,
        start: u64,
        len: u64,
        mode: UffdRegisterMode,
    ) -> Result<UffdIoctlsSupported> {
        let mut reg = UffdioRegister {
            range_start: UffdRange { start, len },
            mode,
            ioctls: UffdIoctlsSupported::empty(),
        };

        // Generate the ioctl wrapper
        nix::ioctl_readwrite!(
            uffd_register_ioctl_cmd,
            uffd_consts::UFFDIO,
            uffd_consts::_UFFDIO_REGISTER as u8,
            UffdioRegister
        );

        let result =
            unsafe { uffd_register_ioctl_cmd(self.as_raw_fd(), &mut reg as *mut UffdioRegister)? };
        if result < 0 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(reg.ioctls)
    }
}

pub fn create_uffd(flags: UffdFlags) -> Result<Uffd> {
    let result = unsafe { syscall(SYS_userfaultfd, flags) };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(unsafe { Uffd::from_raw_fd(result as RawFd) })
}
