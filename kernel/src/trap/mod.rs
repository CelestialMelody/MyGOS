mod context;
mod handler;
pub use context::*;
pub use handler::*;
use riscv::register::scause::{Interrupt, Trap};

use crate::consts::TRAMPOLINE;
use crate::task::trap_context_position;
use crate::task::{current_task, current_user_token};
use crate::timer::{get_timeval, set_next_trigger};
use core::arch::{asm, global_asm};
use riscv::register::{mtvec::TrapMode, sie, stvec};

global_asm!(include_str!("trampoline.S"));

pub fn init() {
    set_kernel_trap_entry();
}

/// 设置内核态下的 trap 入口
///
/// 在内核态发生 trap 后, CPU 会跳转执行 [`kernel_trap_handler`] 处的代码
fn set_kernel_trap_entry() {
    unsafe { stvec::write(kernel_trap_handler as usize, TrapMode::Direct) }
}

/// 设置用户态下的 trap 入口
///
/// 在用户态发生 trap 后, CPU 会跳转执行 [`TRAMPOLINE`] 处的代码
fn set_user_trap_entry() {
    unsafe { stvec::write(TRAMPOLINE as usize, TrapMode::Direct) }
}

/// Enable S-mode timer interrupt.
pub fn enable_stimer_interrupt() {
    unsafe { sie::set_stimer() }
}

#[no_mangle]
pub fn trap_return() -> ! {
    set_user_trap_entry();

    let user_satp = current_user_token();

    extern "C" {
        fn user_trapvec();
        fn user_trapret();
    }

    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    let diff = get_timeval() - inner.last_enter_smode_time;
    inner.add_stime(diff);
    inner.set_last_enter_umode(get_timeval());

    if let Some(scause) = inner.trap_cause {
        if matches!(scause.cause(), Trap::Interrupt(Interrupt::SupervisorTimer)) {
            set_next_trigger();
        }
        inner.trap_cause = None;
    }
    let trap_addr = trap_context_position(task.pid() - task.tgid).0;
    drop(inner);
    drop(task);

    let trapret_addr = user_trapret as usize - user_trapvec as usize + TRAMPOLINE;
    unsafe {
        asm!(
            "fence.i",              // Clear up i-cache.
            "jr {user_trapret}",
            user_trapret = in(reg) trapret_addr,
            in("a0") trap_addr,     // User's trap context virtual address.
            in("a1") user_satp,     // User's memory set token.
            options(noreturn)
        );
    }
}
