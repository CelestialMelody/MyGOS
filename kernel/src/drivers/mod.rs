//! Drivers on BTD-OS, used by [board].
//!
//! [board]: crate::board

use crate::alloc::string::ToString;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use fat32::BlockDevice;

pub mod cvitex;
mod divice;
mod qemu;

#[cfg(feature = "cvitex")]
use crate::{boards::DEVICE_TREE, drivers::cvitex::init_blk_driver};
use alloc::vec::Vec;
#[cfg(feature = "cvitex")]
use cvitex::BlockDeviceImpl;
pub use divice::*;
use fdt::node::FdtNode;
use fdt::Fdt;
#[cfg(feature = "qemu")]
use qemu::BlockDeviceImpl;

pub use divice::*;
use spin::lazy::Lazy;
use spin::Mutex;
pub static DEVICE_SET: Mutex<DeviceSet> = Mutex::new(DeviceSet::new());
pub static DRIVER_REGIONS: Mutex<BTreeMap<&str, fn(&FdtNode) -> Arc<dyn Driver>>> =
    Mutex::new(BTreeMap::new());
pub static BLOCK_DEVICE: Lazy<Arc<dyn BlockDevice>> =
    Lazy::new(|| Arc::new(BlockDeviceImpl::new()));

// pub fn get_blk_device(id: usize) -> Option<Arc<dyn BlkDriver>> {
//     let divice_set = DEVICE_SET.lock();
//     let len = divice_set.blk.len();
//     match id < len {
//         true => Some(divice_set.blk[id].clone()),
//         false => None,
//     }
// }

pub fn prepare_devices(device_tree: usize) {
    #[cfg(feature = "cvitex")]
    let fdt: Fdt<'_> = Fdt::new(DEVICE_TREE.as_ref()).unwrap();
    #[cfg(feature = "qemu")]
    let fdt = unsafe { Fdt::from_ptr(device_tree as *const u8).unwrap() };

    let mut device_set = DEVICE_SET.lock();

    // fdt.memory().regions().for_each(|x| {
    //     let size = x.size.unwrap();
    //     let start = x.starting_address as usize;
    //     let end = x.starting_address as usize + size;
    //     println!("Memory region: 0x{:x} - 0x{:x}", start, end);
    // });

    let node = fdt.all_nodes();

    let driver_manager = DRIVER_REGIONS.lock();
    for child in node {
        if let Some(compatible) = child.compatible() {
            // let info = compatible
            //     .all()
            //     .map(|c| c.to_string())
            //     .collect::<Vec<String>>()
            //     .join(" ");
            // println!("{}:  {}", child.name, info);
            for item in compatible.all() {
                if let Some(f) = driver_manager.get(item) {
                    device_set.add_device(f(&child));
                    break;
                }
            }
        }
    }

    #[cfg(feature = "cvitex")]
    init_blk_driver();
}
