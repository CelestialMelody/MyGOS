use alloc::vec::Vec;
use fdt::standard_nodes::Compatible;
use riscv::register::sstatus;

pub const CLOCK_FREQ: usize = 25000000;
pub const PHYSICAL_MEM_END: usize = 0x8fe0_0000; // 0x8000_0000 + 127MB
pub const PHYSICAL_MEM_BEGIN: usize = 0x8000_0000;

pub static DEVICE_TREE: &[u8] = include_bytes!("cv1812h.dtb");

pub fn init_device() -> usize {
    // 开启SUM位 让内核可以访问用户空间
    unsafe {
        sstatus::set_sum();
    }
    DEVICE_TREE.as_ptr() as usize
}

#[derive(Debug, Clone, Copy)]
pub struct MemRegion {
    pub start: usize,
    pub end: usize,
}

use alloc::string::String;
use fdt::{node, Fdt};
use spin::lazy::Lazy;
use spin::Mutex;

use alloc::string::ToString;
use core::cmp::max;
use core::mem;

pub static MMIO: Mutex<Vec<MemRegion>> = Mutex::new(Vec::new());

pub fn init_mmio() {
    let mut mem_regions = vec![];
    let fdt = Fdt::new(DEVICE_TREE.as_ref()).unwrap();

    let nodes = fdt.all_nodes();

    fdt.all_nodes().for_each(|node| {
        let device_select = node.name.contains('@');
        // println!("name: {}, device_select: {}", node.name, device_select);
        if !device_select {
            return;
        }
        if let Some(regions) = node.reg() {
            regions.for_each(|region| {
                let start = region.starting_address as usize;
                if let Some(size) = region.size {
                    let end = start + size as usize;
                    mem_regions.push(MemRegion { start, end });
                } else {
                    // println!("region size is None, start: {:#X}", start);
                }
            });
        }
    });

    MMIO.lock().clone_from(&mem_regions);
}
