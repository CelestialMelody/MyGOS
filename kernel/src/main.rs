#![no_std]
#![no_main]
// Features, need nightly toolchain.
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(slice_from_ptr_range)]
#![feature(error_in_core)]
#![feature(drain_filter)]
#![allow(dead_code)]
#![allow(unused)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate bitflags;
// TODO 测试硬件是否能使用 lazy_static
// #[macro_use]
// extern crate lazy_static;
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

use crate::drivers::BLOCK_DEVICE;
use core::{arch::global_asm, slice};
use riscv::register::sstatus;

global_asm!(include_str!("entry.S"));

const BANNER: &str = r#"
    __  _____  ____________  _____
   /  |/  /\ \/ / ____/ __ \/ ___/
  / /|_/ /  \  / / __/ / / /\__ \
 / /  / /   / / /_/ / /_/ /___/ /
/_/  /_/   /_/\____/\____//____/

"#;

#[no_mangle]
extern "C" fn main(hartid: usize, device_tree: usize) -> ! {
    // 清理 bss 段
    init_bss();
    // 获取设备树信息
    #[cfg(feature = "cvitex")]
    let device_tree = boards::init_device();

    unsafe {
        // 开启浮点运算
        sstatus::set_fs(sstatus::FS::Dirty);
        kernel_main(hartid, device_tree);
    }
}

#[no_mangle]
pub fn kernel_main(_hartid: usize, _device_tree: usize) -> ! {
    {
        // TODO 目前做的是单核，似乎cv1812h启动默认在1号核心
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
    // #[cfg(feature = "cvitex")]
    // drivers::prepare_devices(device_tree);

    trap::init();
    trap::enable_stimer_interrupt();
    timer::set_next_trigger();

    fs::init();
    task::add_initproc();

    task::run_tasks();
    unreachable!()
}

fn init_bss() {
    extern "C" {
        // fn ekstack0();
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
