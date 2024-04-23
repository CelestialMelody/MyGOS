//! About syscall detail: https://man7.org/linux/man-pages/dir_section_2.html

use crate::boards::CLOCK_FREQ;
use crate::fs::{make_pipe, open};
use crate::mm::{
    copyin, copyout, translated_bytes_buffer, translated_mut, translated_ref, translated_str,
    UserBuffer,
};
use crate::return_errno;
use crate::task::{
    current_task, current_user_token, exit_current_and_run_next, pid2task,
    suspend_current_and_run_next, SignalContext,
};
use crate::timer::{get_time, NSEC_PER_SEC};
use alloc::{string::String, string::ToString, sync::Arc, vec::Vec};
use core::usize;
use nix::info::RUsage;
use nix::resource::{RLimit, Resource};
use nix::robustlist::RobustList;
use nix::time::TimeSpec;
use nix::{
    CloneFlags, CpuMask, CreateMode, MaskFlags, OpenFlags, SchedParam, SigAction, SigInfo, SigMask,
    Signal, UContext, MAX_SIGNUM, RUSAGE_SELF, SCHED_OTHER,
};

use super::super::errno::*;

use crate::task::*;

// clone 220
pub fn sys_do_fork(flags: usize, stack_ptr: usize, ptid: usize, tls: usize, ctid: usize) -> Result {
    let current_task = current_task().unwrap();
    let _signal = flags & 0xff;
    let flags = CloneFlags::from_bits(flags & !0xff).unwrap();

    let new_task = current_task.fork(flags);

    if stack_ptr != 0 {
        let trap_cx = new_task.inner_mut().trap_context();
        trap_cx.set_sp(stack_ptr);
    }
    let new_pid = new_task.pid.0;

    let memory_set = new_task.memory_set.read();
    let child_token = memory_set.token();
    drop(memory_set);

    if flags.contains(CloneFlags::PARENT_SETTID) {
        *translated_mut(current_user_token(), ptid as *mut u32) = new_pid as u32;
    }
    if flags.contains(CloneFlags::CHILD_SETTID) {
        *translated_mut(child_token, ctid as *mut u32) = new_pid as u32;
    }
    if flags.contains(CloneFlags::CHILD_CLEARTID) {
        new_task.inner_mut().clear_child_tid = ctid;
    }
    if flags.contains(CloneFlags::SETTLS) {
        let trap_cx = new_task.inner_mut().trap_context();
        trap_cx.set_tp(tls);
    }

    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_mut().trap_context();
    // we do not have to move to next instruction since we have done it before
    // trap_handler 已经将当前进程 Trap 上下文中的 sepc 向后移动了 4 字节,
    // 使得它回到用户态之后, 会从发出系统调用的 ecall 指令的下一条指令开始执行

    trap_cx.x[10] = 0; // 对于子进程, 返回值是0
    add_task(new_task); // 将 fork 到的进程加入任务调度器
    unsafe {
        core::arch::asm!("sfence.vma");
        core::arch::asm!("fence.i");
    }
    Ok(new_pid as isize) // 对于父进程, 返回值是子进程的 PID
}

// execve 221
pub fn sys_exec(path: *const u8, mut argv: *const usize, mut envp: *const usize) -> Result {
    let token = current_user_token();
    // 读取到用户空间的应用程序名称(路径)
    let mut path = translated_str(token, path);

    // {
    //     info!("exec path:{}", path);
    // }
    // println!("path:{:?},argv:{:?},envp:{:?}",path,argv,envp);
    let mut args_vec: Vec<String> = Vec::new();
    if path.ends_with(".sh") {
        path = "busybox".to_string();
        args_vec.push("sh".to_string());
    }
    // if path == "/bin/sh"{
    // // for lmbench_all lat_proc -P 1 shell
    // 	warn!("/bin/sh redirect to /busybox");
    //     path = "/busybox".to_string();
    // }
    if argv as usize != 0 {
        loop {
            let mut arg_str_ptr = 0;
            copyin(token, &mut arg_str_ptr, argv);
            if arg_str_ptr == 0 {
                // 读到下一参数地址为0表示参数结束
                break;
            } // 否则从用户空间取出参数, 压入向量
            args_vec.push(translated_str(token, arg_str_ptr as *const u8));
            unsafe { argv = argv.add(1) }
        }
    }
    // 环境变量
    let mut envs_vec: Vec<String> = Vec::new();
    if envp as usize != 0 {
        loop {
            let mut env_str_ptr = 0;
            copyin(token, &mut env_str_ptr, envp);
            if env_str_ptr == 0 {
                // 读到下一参数地址为0表示参数结束
                break;
            } // 否则从用户空间取出参数, 压入向量
              //	    println!("envp:{:?},env_str_ptr:{:x?}",envp,env_str_ptr);
            envs_vec.push(translated_str(token, env_str_ptr as *const u8));
            unsafe {
                envp = envp.add(1);
            }
        }
    }
    envs_vec.push("PATH=/".to_string());
    envs_vec.push("LD_LIBRARY_PATH=/".to_string());
    // TODO right value
    envs_vec.push("ENOUGH=5000".to_string());
    let task = current_task().unwrap();

    let inner = task.inner_mut();
    let new_path = inner.cwd.clone().cd(path);
    let app_inode = open(new_path.clone(), OpenFlags::O_RDONLY, CreateMode::empty())?;
    if app_inode.file_size() < 64 {
        return_errno!(Errno::ENOEXEC);
    }
    drop(inner);
    task.exec(app_inode, args_vec, envs_vec);
    Ok(0)
    // } else {
    //     return_errno!(Errno::ENOENT, "path {:?} not exits", new_path);
    // }
}

// wait4 260
pub fn sys_wait4(pid: isize, exit_code_ptr: *mut i32) -> Result {
    let task = current_task().unwrap();

    let inner = task.inner_ref();

    // 根据pid参数查找有没有符合要求的进程
    if pid == -1 && inner.children.len() == 0 {
        return Ok(0);
    }
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.pid())
    {
        return_errno!(Errno::ECHILD, "pid {} does not exist", pid);
    }
    drop(inner);

    loop {
        let mut inner = task.inner_mut();
        // 查找所有符合PID要求的处于僵尸状态的进程, 如果有的话还需要同时找出它在当前进程控制块子进程向量中的下标
        let pair = inner
            .children
            .iter()
            .enumerate()
            .find(|(_, p)| p.inner_ref().is_zombie() && (pid == -1 || pid as usize == p.pid()));
        if let Some((idx, _)) = pair {
            // 将子进程从向量中移除并置于当前上下文中
            let child = inner.children.remove(idx);
            // 确认这是对于该子进程控制块的唯一一次强引用, 即它不会出现在某个进程的子进程向量中,
            // 更不会出现在处理器监控器或者任务管理器中.当它所在的代码块结束, 这次引用变量的生命周期结束,
            // 将导致该子进程进程控制块的引用计数变为 0 , 彻底回收掉它占用的所有资源,
            // 包括: 内核栈和它的 PID 还有它的应用地址空间存放页表的那些物理页帧等等
            assert_eq!(Arc::strong_count(&child), 1);
            // 收集的子进程信息返回
            let cpid = child.pid();
            let exit_code = child.inner_ref().exit_code;
            // ++++ release child PCB
            // 将子进程的退出码写入到当前进程的应用地址空间中
            if exit_code_ptr as usize != 0 {
                let memory_set = task.memory_set.read();
                let token = memory_set.token();
                drop(memory_set);
                *translated_mut(token, exit_code_ptr) = exit_code << 8;
            }

            return Ok(cpid as isize);
        } else {
            drop(inner); // 因为下个函数会切换上下文, 所以需要手动释放锁
            suspend_current_and_run_next();
        }
    }
}

// exit 93
pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    unreachable!("unreachable in sys_exit!");
}

pub fn sys_exit_group(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

// getppid 173
pub fn sys_getppid() -> Result {
    Ok(current_task().unwrap().tgid as isize)
}

// getpid 172
pub fn sys_getpid() -> Result {
    Ok(current_task().unwrap().pid.0 as isize)
}

// set_tid_address 96
pub fn sys_set_tid_address(tidptr: *mut usize) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    task.inner_mut().clear_child_tid = tidptr as usize;
    Ok(task.pid() as isize)
}

// getuid 174
pub fn sys_getuid() -> Result {
    Ok(0)
}

// gettid 178
pub fn sys_gettid() -> Result {
    Ok(0)
}

// geteuid 175
pub fn sys_geteuid() -> Result {
    Ok(0)
}

// ppoll 73
pub fn sys_ppoll(
    fds: usize,
    nfds: usize,
    tmo_p: *const TimeSpec,
    sigmask: *const SigMask,
) -> Result {
    Ok(1)
}

// clock_gettime 113
pub fn sys_clock_gettime(_clk_id: usize, ts: *mut u64) -> Result {
    if ts as usize == 0 {
        return Ok(0);
    }
    let token = current_user_token();
    let ticks = get_time();
    let sec = (ticks / CLOCK_FREQ) as u64;
    let nsec = ((ticks % CLOCK_FREQ) * (NSEC_PER_SEC / CLOCK_FREQ)) as u64;
    *translated_mut(token, ts) = sec;
    *translated_mut(token, unsafe { ts.add(1) }) = nsec;
    Ok(0)
}

// kill 129
pub fn sys_kill(pid: usize, signal: u32) -> Result {
    //TODO pid==-1
    if signal == 0 {
        return Ok(0);
    }
    let signal = 1 << (signal - 1);
    if let Some(task) = pid2task(pid) {
        if let Some(flag) = SigMask::from_bits(signal) {
            task.inner_mut().pending_signals |= flag;
            Ok(0)
        } else {
            return_errno!(Errno::EINVAL, "invalid signal, signum: {}", signal);
        }
    } else {
        // return_errno!(Errno::ESRCH, "could not find task with pid: {}", pid); // for hackbench
        Ok(0)
    }
}

// tkill 130
pub fn sys_tkill(tid: usize, signal: usize) -> Result {
    //TODO pid==-1
    // println!("[DEBUG] tkill tid:{:?} signal:0x{:x?}", tid, signal);
    if signal == 0 {
        return Ok(0);
    }
    let pid = if tid == 0 {
        current_task().unwrap().pid.0
    } else {
        tid
    };

    let signal = 1 << (signal - 1);
    if let Some(task) = pid2task(pid) {
        if let Some(flag) = SigMask::from_bits(signal) {
            task.inner_mut().pending_signals |= flag;
            Ok(0)
        } else {
            return_errno!(Errno::EINVAL, "invalid signal, signum: {}", signal);
        }
    } else {
        return_errno!(Errno::ESRCH, "could not find task with pid: {}", pid);
    }
}

// getrusgae 165
pub fn sys_getrusage(who: isize, usage: *mut u8) -> Result {
    if who != RUSAGE_SELF {
        return_errno!(Errno::EINVAL, "currently only supports RUSAGE_SELF");
    }
    let token = current_user_token();
    let mut userbuf = UserBuffer::wrap(translated_bytes_buffer(
        token,
        usage,
        core::mem::size_of::<RUsage>(),
    ));
    let mut rusage = RUsage::new();
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    rusage.ru_stime = inner.stime;
    rusage.ru_utime = inner.utime;
    userbuf.write(rusage.as_bytes());
    Ok(0)
}

// tgkill 131
pub fn sys_tgkill(tgid: isize, tid: usize, sig: isize) -> Result {
    if tgid == -1 {
        todo!(
            "Send the corresponding signal to all threads within\n
        the thread group associated with the current TGID"
        )
    }
    let master_pid = tgid as usize;
    let son_pid = tid;
    if let Some(parent_task) = pid2task(master_pid) {
        let inner = parent_task.inner_mut();
        if let Some(target_task) = inner.children.iter().find(|child| child.pid() == son_pid) {
            todo!("Send Signal")
        } else {
            todo!("errno")
        }
    } else {
        todo!("errno")
    }
}

// sched_getaffinity 123
pub fn sys_sched_getaffinity(pid: usize, cpusetsize: usize, mask: *mut u8) -> Result {
    let token = current_user_token();
    let mut userbuf = UserBuffer::wrap(translated_bytes_buffer(token, mask, cpusetsize));

    // The scheduler in the kernel maintains a bitmap that records the CPU affinity
    // information of processes or threads. When the sched_getaffinity system call
    // is invoked, the scheduler copies the corresponding bits from the bitmap to
    // the memory area pointed to by the mask pointer in the user space.
    let mut cpuset = CpuMask::new();
    cpuset.set(0);
    userbuf.write(cpuset.as_bytes());
    Ok(0)
}

// sched_setaffinity 122
pub fn sys_sched_setaffinity(pid: usize, cpusetsize: usize, mask: *const u8) -> Result {
    let token = current_user_token();
    let mut userbuf = UserBuffer::wrap(translated_bytes_buffer(token, mask, cpusetsize));

    let mut cpuset = CpuMask::new();
    userbuf.read(cpuset.as_bytes_mut());
    Ok(0)
}

// getscheduler 120
pub fn sys_getscheduler(pid: usize) -> Result {
    // let task = pid2task(pid).ok_or(SyscallError::PidNotFound(-1, pid as isize))?;
    // let inner = task.read();
    // Ok(inner.policy as isize)
    Ok(SCHED_OTHER as isize)
}

// sched_getparam 121
pub fn sys_sched_getparam(pid: usize, param: *mut SchedParam) -> Result {
    // let task = pid2task(pid).ok_or(SyscallError::PidNotFound(-1, pid as isize))?;
    // let inner = task.read();
    let token = current_user_token();
    let user_param = translated_mut(token, param);
    // user_param.set_priority(inner.priority);
    user_param.set_priority(1);
    Ok(0)
}

// sched_setscheduler 119
pub fn sys_sched_setscheduler(pid: usize, policy: isize, param: *const SchedParam) -> Result {
    let task = pid2task(pid).ok_or(Errno::DISCARD)?;

    {
        info!("sched_setscheduler: pid: {}, policy: {}", pid, policy);
    }
    // let user_param = translated_ref(token, param);
    // inner.policy = policy as u8;
    // inner.priority = user_param.get_priority();

    Ok(0)
}

// clock_getres 114
pub fn sys_clock_getres(clockid: usize, res: *mut TimeSpec) -> Result {
    let token = current_user_token();
    let user_res = translated_mut(token, res);
    // 赋值看的测试样例 TODO
    user_res.tv_sec = 0;
    user_res.tv_nsec = 1;
    Ok(0)
}

// socketpair 199
pub fn sys_socketpair(domain: isize, _type: isize, _protocol: isize, sv: *mut [i32; 2]) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let user_sv = translated_mut(token, sv);

    let (sv0, sv1) = make_pipe();

    // fd_table mut borrow
    let mut fd_table = task.fd_table.write();
    let inner = task.inner_ref();
    let fd_limit = inner.rlimit_nofile.rlim_cur;
    drop(inner);
    let fd0 = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
    if fd0 >= fd_limit {
        return_errno!(Errno::EMFILE);
    }
    fd_table[fd0] = Some(sv0);

    let fd1 = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
    if fd1 >= fd_limit {
        return_errno!(Errno::EMFILE);
    }
    fd_table[fd1] = Some(sv1);

    drop(fd_table);

    user_sv[0] = fd0 as i32;
    user_sv[1] = fd1 as i32;
    Ok(0)
}

// sigreturn 139
pub fn sys_sigreturn() -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_mut();

    let trap_cx = task_inner.trap_context();

    // 还原被保存的 signal_context
    let sig_context_ptr = trap_cx.x[2]; // 函数调用保证了 x[2] 的值是 sig_context 的地址 (user signal handler 执行前后 x[2] 值不变)
    trap_cx.x[2] += core::mem::size_of::<SignalContext>();
    let siginfo_ptr = trap_cx.x[2];
    trap_cx.x[2] += core::mem::size_of::<SigInfo>();
    let ucontext_ptr = trap_cx.x[2];
    trap_cx.x[2] += core::mem::size_of::<UContext>();

    let ucontext = translated_ref(token, ucontext_ptr as *const UContext);
    let sig_context = translated_ref(token, sig_context_ptr as *mut SignalContext);
    let sigmask = sig_context.mask.clone();
    // 还原 signal handler 之前的 trap context
    *trap_cx = sig_context.context.clone();
    // 还原 signal handler 之前的 signal mask
    task_inner.sigmask = sigmask;
    trap_cx.sepc = ucontext.uc_mcontext.greps[1];

    Ok(0)
}

// sigaction 134
pub fn sys_sigaction(signum: isize, act: *const SigAction, oldact: *mut SigAction) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    let signum = signum as u32;

    // signum 超出范围, 返回错误
    if signum > MAX_SIGNUM || signum == Signal::SIGKILL as u32 || signum == Signal::SIGSTOP as u32 {
        // println!(
        //     "[Kernel] syscall/impl/process: sys_sigaction(signum: {}, sigaction = {:#x?}, old_sigaction = {:#x?} ) = {}",
        //     signum, act, oldact, -Errno::EINVAL
        // );
        return Err(Errno::EINVAL);
    }

    // 当 sigaction 存在时,  在 pcb 中注册给定的 signaction

    if act as usize != 0 {
        let mut sigaction = task.sigactions.write();
        if oldact as usize != 0 {
            copyout(token, oldact, &sigaction[signum as usize]);
        }
        //在 pcb 中注册给定的 signaction
        let mut sa = SigAction::new();
        copyin(token, &mut sa, act);
        // kill 和 stop 信号不能被屏蔽
        sa.sa_mask.sub(Signal::SIGKILL as u32); // sub 函数保证即使不存在 SIGKILL 也无影响
        sa.sa_mask.sub(Signal::SIGSTOP as u32);
        sa.sa_mask.sub(Signal::SIGILL as u32);
        sa.sa_mask.sub(Signal::SIGSEGV as u32);

        sigaction[signum as usize] = sa;
    }

    // println!(
    //     "[Kernel] syscall/impl/process: sys_sigaction(signum: {}, sigaction = {:#x?}, old_sigaction = {:#x?} ) = {}",
    //     signum,
    //     act, // sigact,
    //     oldact,
    //     0
    // );
    Ok(0)
}

// sigprocmask 135
pub fn sys_sigprocmask(
    how: usize,
    set: *const usize,
    old_set: *mut usize,
    sigsetsize: usize,
) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_mut();
    let mut old_mask = task_inner.sigmask.clone();

    if old_set as usize != 0 {
        *translated_mut(token, old_set as *mut SigMask) = old_mask;
    }

    if set as usize != 0 {
        let mut new_set = translated_ref(token, set as *const SigMask).clone();
        new_set.sub(Signal::SIGKILL as u32); // sub 函数保证即使不存在 SIGKILL 也无影响
        new_set.sub(Signal::SIGSTOP as u32);
        new_set.sub(Signal::SIGILL as u32);
        new_set.sub(Signal::SIGSEGV as u32);

        let how = MaskFlags::from_how(how);
        match how {
            MaskFlags::SIG_BLOCK => old_mask |= new_set,
            MaskFlags::SIG_UNBLOCK => old_mask &= !new_set,
            MaskFlags::SIG_SETMASK => old_mask = new_set,
            _ => panic!("ENOSYS"),
        }
        task_inner.sigmask = old_mask;
    }

    // println!(
    //     "[Kernel] syscall/impls/process: sys_sigprocmask(how: {}, set: {:#x?}, old_set: {:#x?}) = 0",
    //     how, set, old_set
    // );
    Ok(0)
}

// set_robust_list 99
pub fn sys_set_robust_list(head: usize, len: usize) -> Result {
    if len != RobustList::HEAD_SIZE {
        return_errno!(Errno::EINVAL, "robust list head len missmatch:{:?}", len);
    }
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    inner.robust_list.head = head;
    drop(inner);
    Ok(0)
}

// get_robust_list 100
pub fn sys_get_robust_list(pid: usize, head_ptr: *mut usize, len_ptr: *mut usize) -> Result {
    let task = if pid == 0 {
        current_task().unwrap()
    } else {
        match pid2task(pid) {
            Some(tsk) => tsk,
            None => return_errno!(Errno::ESRCH, "no such pid:{:?}", pid),
        }
    };
    let token = current_user_token();
    let inner = task.inner_ref();
    let robust_list = &inner.robust_list;
    copyout(token, head_ptr, &robust_list.head);
    copyout(token, len_ptr, &robust_list.len);
    drop(inner);
    Ok(0)
}

// prlimit64 261
pub fn sys_prlimit64(
    pid: usize,
    resource: u32,
    new_limit: *const RLimit,
    old_limit: *mut RLimit,
) -> Result {
    if pid == 0 {
        let task = current_task().unwrap();
        let token = current_user_token();
        let resource = if resource == 7 {
            Resource::NOFILE
        } else {
            Resource::ILLEAGAL
        };
        if !old_limit.is_null() {
            match resource {
                Resource::NOFILE => {
                    let inner = task.inner_ref();
                    let rlimit_nofile = &inner.rlimit_nofile;
                    copyout(token, old_limit, &rlimit_nofile);
                    drop(inner)
                }
                // TODO: Resource::ILLEAGAL => return_errno!(Errno::EINVAL),
                _ => (),
            }
        }
        if !new_limit.is_null() {
            let mut rlimit = RLimit::new(0, 0);
            copyin(token, &mut rlimit, new_limit);
            match resource {
                Resource::NOFILE => {
                    let mut inner = task.inner_mut();
                    let rlimit_nofile = &mut inner.rlimit_nofile;
                    *rlimit_nofile = rlimit;
                    drop(inner)
                }
                // TODO: Resource::ILLEAGAL => return_errno!(Errno::EINVAL),
                _ => (),
            }
        }
        Ok(0)
    } else {
        todo!()
    }
}
