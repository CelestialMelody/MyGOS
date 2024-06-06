#![no_std]
#![no_main]
// Features, need nightly toolchain.
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(slice_from_ptr_range)]
#![feature(error_in_core)]
#![allow(unused)]
#![allow(dead_code)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

#[cfg(feature = "time-tracer")]
#[macro_use]
extern crate time_tracer;

#[macro_use]
mod macros;
#[macro_use]
mod console;

mod boards;
mod consts;
mod drivers;
mod fs;
mod logging;
mod mm;
mod panic;
mod sbi;
mod syscall;
mod task;
mod timer;
mod trap;

use fat32::device;
// mod fat32;

use crate::{
    drivers::{cvitex::init_blk_driver, BLOCK_DEVICE},
    mm::KERNEL_VMM,
};
use alloc::sync::Arc; // Arc

use sbi::sbi_start_hart;
use spin::lazy::Lazy;

use crate::consts::NCPU;
use core::{arch::global_asm, slice, sync::atomic::AtomicBool};

global_asm!(include_str!("entry.S"));

const BANNER: &str = r#"
    __  _____  ____________  _____
   /  |/  /\ \/ / ____/ __ \/ ___/
  / /|_/ /  \  / / __/ / / /\__ \
 / /  / /   / / /_/ / /_/ /___/ /
/_/  /_/   /_/\____/\____//____/

"#;

static BOOTED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

use riscv::register::sstatus;

#[no_mangle]
extern "C" fn main(hartid: usize, device_tree: usize) -> ! {
    // 清理 bss 段
    init_bss();
    // 获取设备树信息
    #[cfg(feature = "cvitex")]
    let device_tree = boards::init_device();
    #[cfg(feature = "cvitex")]
    println!("hartid: {}, device_tree_addr: {:#x}", hartid, device_tree);

    unsafe {
        // 开启浮点运算
        sstatus::set_fs(sstatus::FS::Dirty);

        kernel_main(hartid, device_tree);
    }
}

// use drivers::cvitex::init_blk_driver;

#[no_mangle]
pub fn kernel_main(hartid: usize, device_tree: usize) -> ! {
    {
        #[cfg(feature = "cvitex")]
        if hartid!() == 0 {
            loop {}
        }
    }

    println!("{}", BANNER);
    println!("Boot hart: {}", hartid!());

    logging::init();
    mm::init_kernel_heap_allocator();
    mm::init();

    // get devices and init
    #[cfg(feature = "cvitex")]
    drivers::prepare_devices(device_tree);

    trap::init();
    trap::enable_stimer_interrupt();
    timer::set_next_trigger();

    fs::init();
    println!("fs init done");

    task::add_initproc();
    println!("initproc added");

    #[cfg(feature = "multi-harts")]
    // BOOTED.store(true, core::sync::atomic::Ordering::Relaxed);
    wake_other_harts_hsm();

    task::run_tasks();
    unreachable!()
}

fn init_bss() {
    extern "C" {
        fn ekstack0();
        fn sbss();
        fn ebss();
    }
    unsafe {
        // let sbss = ekstack0 as usize as *mut u8;
        let sbss = sbss as usize as *mut u8;
        let ebss = ebss as usize as *mut u8;
        slice::from_mut_ptr_range(sbss..ebss)
            .into_iter()
            .for_each(|byte| (byte as *mut u8).write_volatile(0));
    }
}
