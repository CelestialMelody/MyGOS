//! Kernel file system
//!
//! The kernel uniformly borrows the VirtFile provided by the fat32 file system
//! as the object for the kernel to operate files.
//!
//! Due to the fact that files within the kernel actually encapsulate the VirtFile
//! provided by FAT32 into KFile, resulting in data being synchronized to the disk
//! every time it is written (the kernel file system is based on FAT32), it is necessary
//! to create the required files/directories in advance for certain tests that demand
//! the presence of such files in the kernel.
//! A more reasonable solution would be to implement a tempfs within the kernel.
//! However, as we are about to enter the second stage of the national competition,
//! there is currently no time to improve the kernel file system.
//! If future participating teams refer to the code implementation of our file system,
//! we recommend looking at the implementation of the file system in TitanixOS,
//! which was developed by a team from the same competition.
//! In simple terms, TitanixOS implements most of its files within the kernel instead
//! of relying on the FAT32 file system. This allows for significantly faster
//! execution speed during testing in TitanixOS.
//! TitanixOS seems to only read test files/programs from FAT32 filesystems

#[cfg(feature = "fat32")]
mod fat;
mod file;
mod mount;
mod pipe;
#[cfg(feature = "ramfs")]
mod ramfs;
mod stdio;

#[cfg(feature = "fat32")]
pub use self::fat::*;
pub use file::*;
pub use mount::*;
pub use path::*;
pub use pipe::*;
#[cfg(feature = "ramfs")]
pub use ramfs::*;
pub use stdio::*;

// use crate::return_errno;
use crate::syscall::impls::Errno;
// use crate::BLOCK_DEVICE;
// use alloc::string::ToString;
// use alloc::sync::Arc;
// use fat32::{root, Dir as FatDir, DirError, FileSystem, VirtFile, VirtFileType, ATTR_DIRECTORY};
pub use path::*;
// use spin::lazy::Lazy;
// use spin::rwlock::RwLock;
use spin::Mutex;

use crate::mm::UserBuffer;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::fmt::{self, Formatter};
// pub use mount::MNT_TABLE;
use nix::{Dirent, InodeTime, Kstat, OpenFlags};
use path::AbsolutePath;
// pub use pipe::{make_pipe, Pipe};
// pub use stdio::{Stdin, Stdout};

use nix::CreateMode;

pub fn init() {
    open(
        "/proc".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/tmp".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/var".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev/misc".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev/shm".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/var/tmp".into(),
        OpenFlags::O_DIRECTORY | OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open("/dev/null".into(), OpenFlags::O_CREAT, CreateMode::empty()).unwrap();
    open("/dev/zero".into(), OpenFlags::O_CREAT, CreateMode::empty()).unwrap();
    open(
        "/proc/mounts".into(),
        OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/proc/meminfo".into(),
        OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev/misc/rtc".into(),
        OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/var/tmp/lmbench".into(),
        OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();

    // sys_clock_getres
    // 应用程序可以通过打开 /dev/cpu_dma_latency 设备文件, 并向其写入一个非负整数, 来请求将 CPU 切换到低延迟模式.
    // 写入的整数值表示请求的最大延迟时间, 单位为微秒
    open(
        "/dev/cpu_dma_latency".into(),
        OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .unwrap();

    open("/dev/tty".into(), OpenFlags::O_CREAT, CreateMode::empty()).unwrap();
    open("/lat_sig".into(), OpenFlags::O_CREAT, CreateMode::empty()).unwrap();
}

static INO_ALLOCATOR: Mutex<Allocator> = Mutex::new(Allocator::new());
struct Allocator {
    current: u64,
}
impl Allocator {
    pub const fn new() -> Self {
        Allocator { current: 0 }
    }
    fn fetch_add(&mut self) -> u64 {
        let id = self.current;
        self.current += 1;
        id
    }
    pub fn alloc(&mut self) -> u64 {
        self.fetch_add()
    }
}
pub fn ino_alloc() -> u64 {
    INO_ALLOCATOR.lock().alloc()
}

pub trait File: Send + Sync {
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    fn available(&self) -> bool;
    /// 从文件中读取数据放到缓冲区中, 最多将缓冲区填满, 并返回实际读取的字节数
    fn read_to_ubuf(&self, buf: UserBuffer) -> usize;
    /// 将缓冲区中的数据写入文件, 最多将缓冲区中的数据全部写入, 并返回直接写入的字节数
    fn write_from_ubuf(&self, buf: UserBuffer) -> usize;
    fn pread(&self, _buf: UserBuffer, _offset: usize) -> usize {
        panic!("{} not implement pread", self.name());
    }
    fn pwrite(&self, _buf: UserBuffer, _offset: usize) -> usize {
        panic!("{} not implement pwrite", self.name());
    }
    fn read_at_direct(&self, _offset: usize, _len: usize) -> Vec<u8> {
        panic!("{} not implement read_at_direct", self.name());
    }
    fn write_from_direct(&self, _offset: usize, _data: &Vec<u8>) -> usize {
        panic!("{} not implement write_from_direct", self.name());
    }
    fn kernel_read_with_offset(&self, _offset: usize, _len: usize) -> Vec<u8> {
        panic!("{} not implement read_to_kspace_with_offset", self.name());
    }
    fn seek(&self, _pos: usize) {
        panic!("{} not implement seek", self.name());
    }
    fn name(&self) -> String;
    fn fstat(&self, _kstat: &mut Kstat) {
        panic!("{} not implement fstat", self.name());
    }
    fn set_time(&self, _xtime_info: InodeTime) {
        panic!("{} not implement set_time", self.name());
    }
    fn time(&self) -> InodeTime {
        panic!("{} not implement get_time", self.name());
    }
    fn dirent(&self, _dirent: &mut Dirent) -> isize {
        panic!("{} not implement get_dirent", self.name());
    }
    fn getdents(&self, _buf: &mut [u8]) -> Result<isize, Errno> {
        panic!("{} not implement getdents", self.name());
    }
    fn offset(&self) -> usize {
        panic!("{} not implement get_offset", self.name());
    }
    fn set_flags(&self, _flag: OpenFlags) {
        panic!("{} not implement set_flags", self.name());
    }
    fn flags(&self) -> OpenFlags {
        panic!("{} not implement get_flags", self.name());
    }
    fn set_cloexec(&self) {
        panic!("{} not implement set_cloexec", self.name());
    }
    fn read_to_kspace(&self) -> Vec<u8> {
        panic!("{} not implement read_kernel_space", self.name());
    }
    fn write_from_kspace(&self, _data: &Vec<u8>) -> usize {
        panic!("{} not implement write_kernel_space", self.name());
    }
    fn file_size(&self) -> usize {
        panic!("{} not implement file_size", self.name());
    }
    fn r_ready(&self) -> bool {
        true
    }
    fn w_ready(&self) -> bool {
        true
    }
    fn path(&self) -> AbsolutePath {
        unimplemented!("not implemente yet");
    }
    fn truncate(&self, _new_length: usize) {
        unimplemented!("not implemente yet");
    }
    fn fid(&self) -> u64 {
        unimplemented!("not implemente yet");
    }
    fn delete(&self) -> usize {
        unimplemented!("not implemente yet");
    }
    fn rename(&self, _new_name: AbsolutePath, _flags: OpenFlags) {
        unimplemented!("not implemente yet");
    }
    fn is_dir(&self) -> bool {
        unimplemented!("not implemente yet");
    }
}

impl Debug for dyn File + Send + Sync {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "name:{}", self.name())
    }
}
