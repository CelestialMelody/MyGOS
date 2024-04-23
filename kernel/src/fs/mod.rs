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

mod fat32;
mod file;
mod mount;
mod page_cache;
mod pipe;
mod stdio;
pub use self::fat32::*;
pub use file::*;
pub use mount::*;
pub use page_cache::*;
pub use path::*;
pub use pipe::*;
use spin::Mutex;
pub use stdio::*;

mod page;
pub use page::*;

use alloc::string::ToString;
use nix::{CreateMode, OpenFlags};
use path::AbsolutePath;
pub use path::*;

pub use crate::fs::fat32::{chdir, open};
pub use mount::MNT_TABLE;
pub use pipe::{make_pipe, Pipe};
pub use stdio::{Stdin, Stdout};

pub fn init() {
    println!("File system: initializing... 1");
    open(
        "/proc".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    println!("File system: initializing... 2");
    open(
        "/tmp".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/var".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev/misc".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev/shm".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/var/tmp".into(),
        OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open("/dev/null".into(), OpenFlags::O_CREATE, CreateMode::empty()).unwrap();
    open("/dev/zero".into(), OpenFlags::O_CREATE, CreateMode::empty()).unwrap();
    open(
        "/proc/mounts".into(),
        OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/proc/meminfo".into(),
        OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/dev/misc/rtc".into(),
        OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();
    open(
        "/var/tmp/lmbench".into(),
        OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();

    // sys_clock_getres
    // 应用程序可以通过打开 /dev/cpu_dma_latency 设备文件, 并向其写入一个非负整数, 来请求将 CPU 切换到低延迟模式.
    // 写入的整数值表示请求的最大延迟时间, 单位为微秒
    open(
        "/dev/cpu_dma_latency".into(),
        OpenFlags::O_CREATE,
        CreateMode::empty(),
    )
    .unwrap();

    open("/dev/tty".into(), OpenFlags::O_CREATE, CreateMode::empty()).unwrap();
    open("/lat_sig".into(), OpenFlags::O_CREATE, CreateMode::empty()).unwrap();
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
