use crate::fs::File;

use alloc::{borrow::ToOwned, sync::Arc};
use core::arch::global_asm;
use nix::{CreateMode, OpenFlags};
use path::AbsolutePath;
use spin::Mutex;

use crate::{fs::open, task::TaskControlBlock};

#[cfg(feature = "static-busybox")]
pub use static_busybox::*;

global_asm!(include_str!("initproc.S"));

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
        open(path, OpenFlags::O_CREAT, CreateMode::empty()).expect("initproc create failed");
    // inode.write_all(&initproc.to_owned());
    inode.write_from_kspace(&initproc.to_owned());

    let task = Arc::new(TaskControlBlock::new(inode.clone()));
    inode.delete();

    load_test();

    task
});

fn load_test() {
    let lck = TEST.lock();
    drop(lck);
}

pub static TEST: Lazy<Mutex<()>> = Lazy::new(|| {
    extern "C" {
        fn test_all_custom_entry();
        fn test_all_custom_tail();
    }
    let entry = test_all_custom_entry as usize;
    let tail = test_all_custom_tail as usize;
    let siz = tail - entry;
    let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path_test_all = AbsolutePath::from_str("/test_all_custom.sh");
    let inode = open(path_test_all, OpenFlags::O_CREAT, CreateMode::empty())
        .expect("no kernel/src/task/initproc/test_all_custom.sh");
    inode.write_from_kspace(&initproc.to_owned());

    // TODO for ramfs
    extern "C" {
        fn busybox_entry();
        fn busybox_tail();
    }
    let entry = busybox_entry as usize;
    let tail = busybox_tail as usize;
    let siz = tail - entry;
    let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path_busy_box = AbsolutePath::from_str("/busybox");
    let inode = open(path_busy_box, OpenFlags::O_CREAT, CreateMode::empty())
        .expect("no kernel/src/task/initproc/busybox");
    inode.write_from_kspace(&initproc.to_owned());

    extern "C" {
        fn busybox_testcode_entry();
        fn busybox_testcode_tail();
    }
    let entry = busybox_testcode_entry as usize;
    let tail = busybox_testcode_tail as usize;
    let siz = tail - entry;
    let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path_busybox_test = AbsolutePath::from_str("/busybox_testcode.sh");
    let inode = open(path_busybox_test, OpenFlags::O_CREAT, CreateMode::empty())
        .expect("no kernel/src/task/initproc/busybox_testcode.sh");
    inode.write_from_kspace(&initproc.to_owned());

    extern "C" {
        fn busybox_test_cmd_entry();
        fn busybox_test_cmd_tail();
    }
    let entry = busybox_test_cmd_entry as usize;
    let tail = busybox_test_cmd_tail as usize;
    let siz = tail - entry;
    let initproc = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path_busybox_test_cmd = AbsolutePath::from_str("/busybox_cmd.txt");
    let inode = open(
        path_busybox_test_cmd,
        OpenFlags::O_CREAT,
        CreateMode::empty(),
    )
    .expect("no kernel/src/task/initproc/busybox_cmd.txt");
    inode.write_from_kspace(&initproc.to_owned());

    Mutex::new(())
});
#[cfg(feature = "static-busybox")]
mod static_busybox {
    use crate::fs::open;
    use crate::fs::File;
    use crate::mm::MemorySet;
    use crate::task::TaskControlBlock;
    use alloc::vec::Vec;
    use alloc::{borrow::ToOwned, sync::Arc};
    use nix::AuxEntry;
    use nix::CreateMode;
    use nix::OpenFlags;
    use path::AbsolutePath;
    use spin::Lazy;
    use spin::RwLock;
    // This is the processing done in the first stage of the national competition.
    // At that time, the file system was not optimized, and the page cache mechanism was not added.
    // We could only do some simple optimization.
    pub static mut STATIC_BUSYBOX_ENTRY: usize = 0;
    pub static mut STATIC_BUSYBOX_AUX: Vec<AuxEntry> = Vec::new();
    pub struct Busybox {
        inner: Arc<TaskControlBlock>,
    }

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

    pub static BUSYBOX: Lazy<RwLock<Busybox>> = Lazy::new(|| {
        info!("Start BusyBox");
        extern "C" {
            fn busybox_entry();
            fn busybox_tail();
        }
        let entry = busybox_entry as usize;
        let tail = busybox_tail as usize;
        let siz = tail - entry;

        let busybox = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
        let path = AbsolutePath::from_str("/static-busybox");

        let inode = open(path, OpenFlags::O_CREAT, CreateMode::empty())
            .expect("static-busybox create failed");
        inode.write_from_kspace(&busybox.to_owned());

        let task = Arc::new(TaskControlBlock::new(inode.clone()));
        inode.delete();

        RwLock::new(Busybox { inner: task })
    });
}
