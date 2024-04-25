//! Drivers on BTD-OS, used by [board].
//!
//! [board]: crate::board

use crate::{alloc::string::ToString, drivers::cvitex::init_blk_driver};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
// use fat32::BlockDevice;
use crate::fat32::BlockDevice;

mod cv1811h_sd;
mod cvitex;
mod fu740;
mod qemu;

#[cfg(feature = "cvitex")]
use cvitex::BlockDeviceImpl;
#[cfg(feature = "fu740")]
use fu740::BlockDeviceImpl;
#[cfg(feature = "qemu")]
use qemu::BlockDeviceImpl;

use simple_sync::LazyInit;
// lazy_static! {
// pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
// }
use spin::lazy::Lazy;
pub static BLOCK_DEVICE: Lazy<Arc<dyn BlockDevice>> = Lazy::new(|| {
    println!("init block device");
    let ret = Arc::new(BlockDeviceImpl::new());
    println!("init block device done");
    ret
});

pub static DEVICE_TREE: LazyInit<Vec<u8>> = LazyInit::new();

/// Initialize platform specific device drivers.
#[cfg(feature = "cvitex")]
pub fn init(device_tree: usize) {
    let fdt = unsafe { Fdt::from_ptr(device_tree as *const u8).unwrap() };
    let mut device_tree_buf = vec![0u8; fdt.total_size()];

    device_tree_buf.copy_from_slice(unsafe {
        core::slice::from_raw_parts(device_tree as *const u8, fdt.total_size())
    });
    DEVICE_TREE.init_with(device_tree_buf);
}

mod divice;

pub use divice::*;
use fdt::{node::FdtNode, Fdt};
use spin::Mutex;
// pub static DIVICE_SET: Lazy<Mutex<DeviceSet>> = Lazy::new(|| Mutex::new(DeviceSet::new()));
pub static DEVICE_SET: Mutex<DeviceSet> = Mutex::new(DeviceSet::new());

pub static DRIVER_REGIONS: Mutex<BTreeMap<&str, fn(&FdtNode) -> Arc<dyn Driver>>> =
    Mutex::new(BTreeMap::new());

// pub fn init_drivers(node: &FdtNode) {
//     let driver_manager = DRIVER_REGIONS.lock();
//     if let Some(compatible) = node.compatible() {
//         let info = compatible
//             .all()
//             .map(|c| c.to_string())
//             .collect::<Vec<String>>()
//             .join(" ");
//         println!("{}:  {}", node.name, info);
//         for item in compatible.all() {
//             if let Some(f) = driver_manager.get(item) {
//                 DEVICE_SET.lock().add_device(f(&node));
//                 break;
//             }
//         }
//     }
// }

#[inline]
pub fn get_blk_device(id: usize) -> Option<Arc<dyn BlkDriver>> {
    let divice_set = DEVICE_SET.lock();
    let len = divice_set.blk.len();
    match id < len {
        true => Some(divice_set.blk[id].clone()),
        false => None,
    }
}

use alloc::vec::Vec;

#[inline]
pub fn get_blk_devices() -> Vec<Arc<dyn BlkDriver>> {
    DEVICE_SET.lock().blk.clone()
}

pub fn prepare_devices() {
    let mut device_set = DEVICE_SET.lock();
    let fdt = Fdt::new(DEVICE_TREE.as_ref()).unwrap();
    println!("There has {} CPU(s)", fdt.cpus().count());

    fdt.memory().regions().for_each(|x| {
        let size = x.size.unwrap();
        let start = x.starting_address as usize;
        let end = x.starting_address as usize + size;

        println!("memory region {:#X} - {:#X}", start, end);
    });

    let node = fdt.all_nodes();

    // for f in DRIVERS_INIT {
    //     f().map(|device| device_set.add_device(device));
    // }

    let driver_manager = DRIVER_REGIONS.lock();
    for child in node {
        if let Some(compatible) = child.compatible() {
            let info = compatible
                .all()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(" ");
            // println!("{}:  {}", child.name, info);
            for item in compatible.all() {
                if let Some(f) = driver_manager.get(item) {
                    println!("add device: {}  {}, info: {}", child.name, item, info);
                    device_set.add_device(f(&child));
                    break;
                }
            }
        }
    }

    // sd card driver
    println!("test 1");
    // device_set.add_device(init_blk_driver());
    init_blk_driver();
    println!("test 2");
    // // register the drivers in the IRQ MANAGER.
    // if let Some(plic) = INT_DEVICE.try_get() {
    //     for (irq, driver) in IRQ_MANAGER.lock().iter() {
    //         plic.register_irq(*irq, driver.clone());
    //     }
    // }
}
