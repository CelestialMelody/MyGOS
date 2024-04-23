use alloc::vec::Vec;
use riscv::register::sstatus;

pub const CLOCK_FREQ: usize = 25000000;
pub const PHYSICAL_MEM_END: usize = 0x8fe0_0000; // 0x8000_0000 + 127MB

pub static DEVICE_TREE: &[u8] = include_bytes!("cv1812h.dtb");

pub fn init_device(hartid: usize, _device_tree: usize) -> (usize, usize) {
    // 开启SUM位 让内核可以访问用户空间  踩坑：
    // only in qemu. eg: qemu is riscv 1.10  NOTE: k210 is riscv 1.9.1
    // in 1.10 is SUM but in 1.9.1 is PUM which is the opposite meaning with SUM
    unsafe {
        sstatus::set_sum();
    }
    (hartid, DEVICE_TREE.as_ptr() as usize)
}

#[derive(Debug, Clone, Copy)]
pub struct MemRegion {
    pub start: usize,
    pub end: usize,
}

use fdt::Fdt;
use spin::lazy::Lazy;
use spin::Mutex;

pub static MMIO: Mutex<Vec<MemRegion>> = Mutex::new(Vec::new());

pub fn init_mmio() {
    let mut mem_regions = vec![];
    let fdt = Fdt::new(DEVICE_TREE.as_ref()).unwrap();

    fdt.memory().regions().for_each(|mr| {
        mem_regions.push(MemRegion {
            start: mr.starting_address as usize,
            end: mr.starting_address as usize + mr.size.unwrap_or(0),
        })
    });

    MMIO.lock().clone_from(&mem_regions);
}
