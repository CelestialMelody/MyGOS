use super::{set_kernel_trap_entry, trap_return};
use crate::mm::VirtAddr;
use crate::syscall::SYS_SIGRETURN;
use crate::{
    consts::TRAMPOLINE,
    syscall::dispatcher::syscall,
    task::{
        current_add_signal, current_task, current_trap_cx, exec_signal_handlers,
        suspend_current_and_run_next,
    },
    timer::{check_interval_timer, get_timeval},
};
use nix::SigSet;
use riscv::register::{mcause, mtval};
use riscv::register::{
    scause::{self, Exception, Interrupt, Trap},
    stval,
};

#[no_mangle]
pub fn user_trap_handler() -> ! {
    // let pid = current_task().unwrap().pid(); println!("pid:{:?}",pid);
    set_kernel_trap_entry();
    // 用于描述 Trap 的原因
    let scause = scause::read();
    let stval = stval::read();
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();

    inner.trap_cause = Some(scause);

    let diff = get_timeval() - inner.last_enter_umode_time;
    inner.add_utime(diff);
    inner.set_last_enter_smode(get_timeval());
    drop(inner);
    drop(task);
    let mut is_sigreturn = false;

    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            let syscall_id = cx.x[17];
            // get system call return value
            if syscall_id == SYS_SIGRETURN {
                is_sigreturn = true;
            }
            let result = syscall(
                cx.a7(),
                [cx.a0(), cx.a1(), cx.a2(), cx.a3(), cx.a4(), cx.a5()],
            );
            // cx is changed during sys_exec, so we have to call it again
            if !is_sigreturn {
                cx = current_trap_cx();
                cx.x[10] = result as usize;
            }
        }

        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            // println!("user_trap_handler: memory fault, task: {} at {:x?}, {:x?}",current_task().pid(),VirtAddr::from(stval as usize),current_trap_cx().sepc);
            let va: VirtAddr = (stval as usize).into();
            if va > TRAMPOLINE.into() {
                // println!("[kernel trap] VirtAddr out of range!");
                current_add_signal(SigSet::SIGSEGV);
            }
            let task = current_task().unwrap();
            let lazy = task.check_lazy(va);
            if lazy != 0 {
                current_add_signal(SigSet::SIGSEGV);
            }
        }

        Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault) => {
            let task = current_task();
            // debug!("{:?} in application {}, bad addr = {:#x}, bad instruction = {:#x}.",scause.cause(),task.pid.0,stval,current_trap_cx().sepc);
            drop(task);
            current_add_signal(SigSet::SIGSEGV);
        }

        Trap::Exception(Exception::IllegalInstruction) => {
            // let sepc = riscv::register::sepc::read();
            // println!("[Kernel] IllegalInstruction at 0x{:x}, stval:0x{:x}", sepc, stval);
            current_add_signal(SigSet::SIGILL);
        }

        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            suspend_current_and_run_next();
            // set_next_trigger(); // we have deal with it in trap_return
        }

        _ => panic!(
            "trap {:?} is unsupported, stval = {:#x}!",
            scause.cause(),
            stval
        ),
    }

    check_interval_timer();

    if !is_sigreturn {
        exec_signal_handlers();
    }

    trap_return();
}

#[no_mangle]
pub fn kernel_trap_handler() -> ! {
    let scause = scause::read();
    let stval = stval::read();
    let sepc = riscv::register::sepc::read();
    let mcause = mcause::read();
    let mtval = mtval::read();
    let mepc = riscv::register::mepc::read();
    println!(
        "[kernel_trap_handler] scause: {:x} {:?}, stval: {:x}, sepc: {:x}",
        scause.bits(),
        scause.cause(),
        stval,
        sepc
    );
    println!(
        "[kernel_trap_handler] mcause: {:x} {:?}, mtval: {:x}, mepc: {:x}",
        mcause.bits(),
        mcause.cause(),
        mtval,
        mepc
    );

    panic!("hart {} nested trap!", hartid!());
}
