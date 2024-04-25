mod context;
mod id;
mod initproc;
mod kstack;
mod manager;
mod processor;
mod signal;
mod switch;
mod task;
pub use context::*;
pub use id::*;
pub use initproc::*;
pub use kstack::*;
pub use manager::*;
use nix::{SAFlags, SigInfo, SigSet, Signal, UContext, SIG_DFL, SIG_IGN};
pub use processor::*;
pub use signal::*;
pub use switch::*;
pub use task::*;

use crate::{
    consts::SIGNAL_TRAMPOLINE,
    mm::{copyout, translated_mut},
    syscall::impls::futex::futex_wake,
};
use alloc::sync::Arc;
// use fat32::sync_all;
use crate::fat32::sync_all;

pub use self::{
    initproc::INITPROC,
    processor::{acquire_processor, schedule},
};

pub fn suspend_current_and_run_next() -> isize {
    exec_signal_handlers();

    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    let task_cx_ptr = &mut inner.task_cx as *mut TaskContext;
    inner.task_status = TaskStatus::Ready;
    drop(inner);

    schedule(task_cx_ptr);
    0
}

pub fn exit_current_and_run_next(exit_code: i32) {
    let task = take_current_task().unwrap();

    let pid = task.pid();
    let token = task.token();
    let is_child_thread = task.is_child_thread();

    remove_from_pid2task(pid);

    let mut inner = task.inner_mut();
    let clear_child_tid = inner.clear_child_tid;
    if clear_child_tid != 0 {
        *translated_mut(token, clear_child_tid as *mut usize) = 0;
        futex_wake(clear_child_tid, 1).unwrap();
    }
    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;

    if pid == 0 {
        sync_all();
        panic!("initproc return!");
    }

    assert!(if is_child_thread {
        inner.children.is_empty()
    } else {
        true
    });

    // Move the child of this process to the child of the initproc process.
    for child in inner.children.iter() {
        // There is no need to distinguish between child threads and child processes.
        // Transfer all children to initproc, and further processing will be done for child threads.
        let mut initproc_inner = INITPROC.inner_mut();
        child.inner_mut().parent = Some(Arc::downgrade(&INITPROC));
        initproc_inner.children.push(child.clone());
    }

    if is_child_thread {
        let parent = inner.parent.as_ref().unwrap().upgrade().unwrap();
        let mut parent_inner = parent.inner_mut();
        let children_iter = parent_inner.children.iter();
        let (idx, _) = children_iter
            .enumerate()
            .find(|(_, t)| t.pid() == pid)
            .unwrap();
        parent_inner.children.remove(idx);
        drop(parent_inner);
        drop(parent);
        drop(inner);
        assert!(Arc::strong_count(&task) == 1);
        collect_cancelled_chiled_thread(task);
        schedule(&mut TaskContext::empty() as *mut _);
        unreachable!()
    }

    drop(inner);
    drop(task);
    schedule(&mut TaskContext::empty() as *mut _);
}

pub fn hanging_current_and_run_next(sleep_time: usize, duration: usize) {
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    let current_cx_ptr = &mut inner.task_cx as *mut TaskContext;
    inner.task_status = TaskStatus::Hanging;
    drop(inner);
    drop(task);
    acquire_processor().hang_current(sleep_time, duration);
    schedule(current_cx_ptr);
}

pub fn block_current_and_run_next() {
    let task = current_task().unwrap();

    let mut task_inner = task.inner_mut();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Blocking;
    block_task(task.clone());

    drop(task_inner);
    drop(task);

    schedule(task_cx_ptr);
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}

pub fn exec_signal_handlers() {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_mut();

    if task_inner.pending_signals == SigSet::empty() {
        return;
    }

    loop {
        // take out the first signal of pending
        let signum = match task_inner
            .pending_signals
            .difference(task_inner.sigmask)
            .fetch()
        {
            Some(s) => s,
            None => return,
        };
        task_inner.pending_signals.sub(signum);
        let sigaction = task.sigactions.read()[signum as usize];

        // if signal handler exists, then prepare to jump to handler
        let handler = sigaction.sa_handler;
        match handler {
            SIG_IGN => {
                // return;
                continue; // loop
            }
            SIG_DFL => {
                if signum == Signal::SIGKILL as u32 || signum == Signal::SIGSEGV as u32 {
                    // info!("[Kernel] task/mod(exec_signal_handlers) pid:{} signal_num:{}, SIG_DFL kill process", pid, signum);
                    drop(task_inner);
                    drop(task);
                    exit_current_and_run_next(-(signum as i32));
                }
                return;
            }
            _ => {
                // block the current signal and the signals in sigaction.sa_mask
                let mut sigmask = sigaction.sa_mask.clone();
                if !sigaction.sa_flags.contains(SAFlags::SA_NODEFER) {
                    sigmask.add(signum);
                }
                // save the old sigmask
                let old_sigmask = task_inner.sigmask.clone();
                sigmask.add_other(old_sigmask);
                // set the signal mask to sigmask
                task_inner.sigmask = sigmask;
                // put the SignalContext data into the stack.
                let trap_cx = task_inner.trap_context();
                // save the trap context and old_sigmask to sig_context
                let sig_context = SignalContext::from_another(trap_cx, old_sigmask);
                trap_cx.x[10] = signum as usize; // a0 (args0 = signum)

                // If SA_SIGINFO is included in sa_flags, put siginfo and ucontext into the stack.
                // However, we did not differentiate whether the flag is present or not. We handled it uniformly.

                let memory_set = task.memory_set.read();
                let token = memory_set.token();
                drop(memory_set);

                trap_cx.x[2] -= core::mem::size_of::<UContext>(); // sp -= sizeof(ucontext)
                let ucontext_ptr = trap_cx.x[2];
                trap_cx.x[2] -= core::mem::size_of::<SigInfo>(); // sp -= sizeof(siginfo)
                let siginfo_ptr = trap_cx.x[2];

                trap_cx.x[11] = siginfo_ptr; // a1 (args1 = siginfo)
                trap_cx.x[12] = ucontext_ptr; // a2 (args2 = ucontext)
                let mut ucontext = UContext::empty();
                ucontext.uc_mcontext.greps[1] = trap_cx.sepc; //pc
                copyout(token, ucontext_ptr as *mut UContext, &ucontext);

                trap_cx.x[2] -= core::mem::size_of::<SignalContext>(); // sp -= sizeof(sigcontext)
                let sig_context_ptr = trap_cx.x[2] as *mut SignalContext;
                copyout(token, sig_context_ptr, &sig_context);

                trap_cx.x[1] = SIGNAL_TRAMPOLINE; // ra = user_sigreturn

                trap_cx.sepc = handler; // sepc = handler
                return;
            }
        }
    }
}
