use core::arch::global_asm;

use alloc::{borrow::ToOwned, sync::Arc};
use nix::{CreateMode, OpenFlags};
use path::AbsolutePath;
use spin::Mutex;

use crate::{fs::open, task::TaskControlBlock};

global_asm!(include_str!("initproc.S"));

// lazy_static! {
//     pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
//         extern "C" {
//             fn initproc_entry();
//             fn initproc_tail();
//         }
//         let entry = initproc_entry as usize;
//         let tail = initproc_tail as usize;
//         let siz = tail - entry;

//         let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
//         let path = AbsolutePath::from_str("/initproc");

//         let  inode = open(path, OpenFlags::O_CREATE, CreateMode::empty()).expect("initproc create failed!");
//         inode.write_all(&initproc.to_owned());

//         let task = TaskControlBlock::new(inode.clone());
//         inode.delete(); // 删除 initproc 文件

//         load_test_all_custom();

//         task
//     });

// }

use spin::lazy::Lazy;
pub static INITPROC: Lazy<Arc<TaskControlBlock>> = Lazy::new(|| {
    extern "C" {
        fn initproc_entry();
        fn initproc_tail();
    }
    let entry = initproc_entry as usize;
    let tail = initproc_tail as usize;
    let siz = tail - entry;

    let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path = AbsolutePath::from_str("/initproc");

    let inode =
        open(path, OpenFlags::O_CREATE, CreateMode::empty()).expect("initproc create failed");
    inode.write_all(&initproc.to_owned());

    let task = Arc::new(TaskControlBlock::new(inode.clone()));
    inode.delete();

    load_test_all_custom();

    task
});

// This is the processing done in the first stage of the national competition.
// At that time, the file system was not optimized, and the page cache mechanism was not added.
// We could only do some simple optimization.
#[cfg(feature = "static-busybox")]
pub static mut STATIC_BUSYBOX_ENTRY: usize = 0;
#[cfg(feature = "static-busybox")]
pub static mut STATIC_BUSYBOX_AUX: Vec<AuxEntry> = Vec::new();
#[cfg(feature = "static-busybox")]
pub struct Busybox {
    inner: Arc<TaskControlBlock>,
}
#[cfg(feature = "static-busybox")]
use crate::mm::MemorySet;
#[cfg(feature = "static-busybox")]
use alloc::vec::Vec;
#[cfg(feature = "static-busybox")]
impl Busybox {
    pub fn elf_entry_point(&self) -> usize {
        unsafe { STATIC_BUSYBOX_ENTRY }
    }
    pub fn aux(&self) -> Vec<AuxEntry> {
        unsafe { STATIC_BUSYBOX_AUX.clone() }
    }
    pub fn memory_set(&self) -> MemorySet {
        let mut memory_set = self.inner.memory_set.write();
        MemorySet::from_copy_on_write(&mut memory_set)
    }
}
#[cfg(feature = "static-busybox")]
use nix::AuxEntry;
#[cfg(feature = "static-busybox")]
use spin::RwLock;
#[cfg(feature = "static-busybox")]
// lazy_static! {
//     pub static ref BUSYBOX: RwLock<Busybox> = RwLock::new({
//         info!("Start Static BusyBox");
//         extern "C" {
//             fn busybox_entry();
//             fn busybox_tail();
//         }
//         let entry = busybox_entry as usize;
//         let tail = busybox_tail as usize;
//         let siz = tail - entry;

//         let busybox = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
//         let path = AbsolutePath::from_str("/static-busybox");

//         let inode = open(path, OpenFlags::O_CREATE, CreateMode::empty())
//             .expect("static-busybox create failed");
//         inode.write_all(&busybox.to_owned());

//         let task = Arc::new(TaskControlBlock::new(inode.clone()));
//         inode.delete();
//         Busybox { inner: task }
//     });
// }
pub static BUSYBOX: Lazy<RwLock<Busybox>> = Lazy::new(|| {
    info!("Start Static BusyBox");
    extern "C" {
        fn busybox_entry();
        fn busybox_tail();
    }
    let entry = busybox_entry as usize;
    let tail = busybox_tail as usize;
    let siz = tail - entry;

    let busybox = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path = AbsolutePath::from_str("/static-busybox");

    let inode =
        open(path, OpenFlags::O_CREATE, CreateMode::empty()).expect("static-busybox create failed");
    inode.write_all(&busybox.to_owned());

    let task = Arc::new(TaskControlBlock::new(inode.clone()));
    inode.delete();

    RwLock::new(Busybox { inner: task })
});

fn load_test_all_custom() {
    let lck = TEST_ALL_CUSTOM.lock();
    drop(lck);
}

// lazy_static! {
//     pub static ref TEST_ALL_CUSTOM: Mutex<()> = Mutex::new({
//         extern "C" {
//             fn test_all_custom_entry();
//             fn test_all_custom_tail();
//         }
//         let entry = test_all_custom_entry as usize;
//         let tail = test_all_custom_tail as usize;
//         let siz = tail - entry;

//         let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
//         let path = AbsolutePath::from_str("/test_all_custom.sh");

//         let inode = open(path, OpenFlags::O_CREATE, CreateMode::empty())
//             .expect("no kernel/src/task/initproc/test_all_custom.sh");
//         inode.write_all(&initproc.to_owned());
//     });
// }

pub static TEST_ALL_CUSTOM: Lazy<Mutex<()>> = Lazy::new(|| {
    extern "C" {
        fn test_all_custom_entry();
        fn test_all_custom_tail();
    }
    let entry = test_all_custom_entry as usize;
    let tail = test_all_custom_tail as usize;
    let siz = tail - entry;

    let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path = AbsolutePath::from_str("/test_all_custom.sh");

    let inode = open(path, OpenFlags::O_CREATE, CreateMode::empty())
        .expect("no kernel/src/task/initproc/test_all_custom.sh");
    inode.write_all(&initproc.to_owned());

    Mutex::new(())
});
