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
use sbi::sbi_start_hart;
use spin::lazy::Lazy;

use crate::consts::NCPU;
use core::{arch::global_asm, slice, sync::atomic::AtomicBool};

global_asm!(include_str!("entry.S"));

const BANNER: &str = r#"
 ____  _ _    _______ _          _____  _     _
|  _ \(_) |  |__   __| |        |  __ \(_)   | |
| |_) |_| |_ ___| |  | |__   ___| |  | |_ ___| | __
|  _ <| | __/ _ \ |  | '_ \ / _ \ |  | | / __| |/ /
| |_) | | ||  __/ |  | | | |  __/ |__| | \__ \   <
|____/|_|\__\___|_|  |_| |_|\___|_____/|_|___/_|\_\
"#;

// lazy_static! {
//     static ref BOOTED: AtomicBool = AtomicBool::new(false);
// }

static BOOTED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

use riscv::register::sstatus;

#[no_mangle]

extern "C" fn main(hartid: usize, device_tree: usize) -> ! {
    init_bss();
    let (hartid, device_tree) = boards::init_device(hartid, device_tree);
    println!("hartid: {}, device_tree: {:#x}", hartid, device_tree);

    // 开启 SUM
    unsafe {
        // 开启浮点运算
        sstatus::set_fs(sstatus::FS::Dirty);

        meow(hartid, device_tree);
    }
}

#[no_mangle]
pub fn meow(hartid: usize, device_tree: usize) -> ! {
    // if BOOTED.load(core::sync::atomic::Ordering::Relaxed) {
    //     other_harts()
    // }

    {
        #[cfg(feature = "cvitex")]
        if hartid!() == 0 {
            loop {}
        }
    }

    println!("{}", BANNER);
    println!("Boot hart: {}", hartid!());

    logging::init();
    println!("logging init done");

    mm::init_kernel_heap_allocator();
    println!("Kernel heap allocator initialized");

    drivers::init(device_tree);
    println!("drivers init done");

    mm::init();
    println!("mm init done");

    // get devices and init
    drivers::prepare_devices();
    println!("devices prepare done");

    trap::init();
    println!("trap init done");

    trap::enable_stimer_interrupt();
    println!("stimer interrupt enabled");

    timer::set_next_trigger();
    println!("timer set next trigger");

    fs::init();
    println!("fs init done");

    task::add_initproc();
    println!("initproc added");

    // BOOTED.store(true, core::sync::atomic::Ordering::Relaxed);
    #[cfg(feature = "multi-harts")]
    wake_other_harts_hsm();

    task::run_tasks();
    println!("task run tasks done");
    unreachable!()
}

fn wake_other_harts_hsm() {
    extern "C" {
        fn _entry();
    }
    let boot_hartid = hartid!();
    for i in 1..NCPU {
        sbi_start_hart((boot_hartid + i) % NCPU, _entry as usize, 0).unwrap();
    }
}

#[allow(unused)]
fn wake_other_harts_ipi() {
    use sbi::sbi_send_ipi;
    let boot_hart = hartid!();
    let target_harts_mask = ((1 << NCPU) - 1) ^ boot_hart;
    sbi_send_ipi(target_harts_mask, (&target_harts_mask) as *const _ as usize).unwrap();
}

fn other_harts() -> ! {
    info!("hart {} has been started", hartid!());
    mm::enable_mmu();
    trap::init();
    trap::enable_stimer_interrupt();
    timer::set_next_trigger();
    task::run_tasks();
    unreachable!()
}

fn init_bss() {
    extern "C" {
        fn ekstack0();
        fn ebss();
    }
    unsafe {
        let sbss = ekstack0 as usize as *mut u8;
        let ebss = ebss as usize as *mut u8;
        slice::from_mut_ptr_range(sbss..ebss)
            .into_iter()
            .for_each(|byte| (byte as *mut u8).write_volatile(0));
    }
}
