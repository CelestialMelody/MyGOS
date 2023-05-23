//! 进程相关系统调用

use crate::fs::open_flags::CreateMode;
use crate::fs::{open, OpenFlags};
use crate::mm::{translated_mut, translated_ref, translated_str};
use crate::task::{
    add_task, current_task, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next,
};
pub use crate::task::{CloneFlags, Utsname, UTSNAME};

use alloc::{string::String, sync::Arc, vec::Vec};

/// #define SYS_clone 220
///
/// 功能：创建一个子进程；
///
/// 输入：
///
/// - flags: 创建的标志，如SIGCHLD；
/// - stack: 指定新进程的栈，可为0；
/// - ptid: 父线程ID；
/// - tls: TLS线程本地存储描述符；
/// - ctid: 子线程ID；
///
/// 返回值：成功则返回子进程的线程ID，失败返回-1；
///
/// ```c
/// pid_t ret = syscall(SYS_clone, flags, stack, ptid, tls, ctid)
/// ```
pub fn sys_clone(flags: usize, stack: usize, ptid: usize, tls: usize, ctid: usize) -> isize {
    todo!()
}

/// #define SYS_execve 221
///
/// 功能：执行一个指定的程序；
///
/// 输入：
///
/// - path: 待执行程序路径名称，
/// - argv: 程序的参数，
/// - envp: 环境变量的数组指针
///
/// 返回值：成功不返回，失败返回-1；
///
/// ```c
/// const char *path, char *const argv[], char *const envp[];
/// int ret = syscall(SYS_execve, path, argv, envp);
/// ```
pub fn sys_execve(path: *const u8, argv: *const u8, envp: *const u8) -> isize {
    todo!()
}

/// #define SYS_wait4 260
///
/// 功能：等待进程改变状态;
///
/// 输入：
///
/// - pid: 指定进程ID，可为-1等待任何子进程；
/// - status: 接收状态的指针；
/// - options: 选项：WNOHANG，WUNTRACED，WCONTINUED；
///
/// 返回值：成功则返回进程ID；如果指定了WNOHANG，且进程还未改变状态，直接返回0；失败则返回-1；
///
/// ```c
/// pid_t pid, int *status, int options;
/// pid_t ret = syscall(SYS_wait4, pid, status, options);
/// ```
pub fn sys_wait4(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();

    let inner = task.lock();

    // 根据pid参数查找有没有符合要求的进程
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.pid())
    {
        return -1;
    }
    drop(inner);

    loop {
        let mut inner = task.lock();
        // 查找所有符合PID要求的处于僵尸状态的进程，如果有的话还需要同时找出它在当前进程控制块子进程向量中的下标
        let pair = inner.children.iter().enumerate().find(|(_, p)| {
            // ++++ temporarily access child PCB lock exclusively
            p.lock().is_zombie() && (pid == -1 || pid as usize == p.pid())
            // ++++ release child PCB
        });
        if let Some((idx, _)) = pair {
            // 将子进程从向量中移除并置于当前上下文中
            let child = inner.children.remove(idx);
            // 确认这是对于该子进程控制块的唯一一次强引用，即它不会出现在某个进程的子进程向量中，
            // 更不会出现在处理器监控器或者任务管理器中。当它所在的代码块结束，这次引用变量的生命周期结束，
            // 将导致该子进程进程控制块的引用计数变为 0 ，彻底回收掉它占用的所有资源，
            // 包括：内核栈和它的 PID 还有它的应用地址空间存放页表的那些物理页帧等等
            // println!("[KERNEL] pid {} waitpid {}",current_task().unwrap().pid.0, pid);
            assert_eq!(Arc::strong_count(&child), 1);
            // 收集的子进程信息返回
            let found_pid = child.pid();
            // ++++ temporarily access child TCB exclusively
            let exit_code = child.lock().exit_code;
            // ++++ release child PCB
            // 将子进程的退出码写入到当前进程的应用地址空间中
            if exit_code_ptr as usize != 0 {
                *translated_mut(inner.memory_set.token(), exit_code_ptr) = exit_code << 8;
            }
            return found_pid as isize;
        } else {
            // 如果找不到的话则放权等待
            drop(inner); // 手动释放 TaskControlBlock 全局可变部分
            suspend_current_and_run_next();
        }
        // ---- release current PCB lock automatically
    }
}

/// #define SYS_exit 93
///
/// 功能：触发进程终止，无返回值；
///
/// 输入：终止状态值；
///
/// 返回值：无返回值；
///
/// ```c
/// int ec;
/// syscall(SYS_exit, ec);
/// ```
pub fn sys_exit(exit_code: i32) -> ! {
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// #define SYS_getppid 173
///
/// 功能：获取父进程ID；
///
/// 输入：系统调用ID；
///
/// 返回值：成功返回父进程ID；
///
/// ```c
/// pid_t ret = syscall(SYS_getppid);
/// ```
pub fn sys_getppid() -> isize {
    current_task().unwrap().tgid as isize
}

/// #define SYS_getpid 172
///
/// 功能：获取进程ID；
///
/// 输入：系统调用ID；
///
/// 返回值：成功返回进程ID；
///
/// ```c
/// pid_t ret = syscall(SYS_getpid);
/// ```
pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// #define SYS_clone 220
///
/// 功能：创建一个子进程；
///
/// 输入：
///
/// - flags: 创建的标志，如SIGCHLD；
/// - stack: 指定新进程的栈，可为0；
/// - ptid: 父线程ID；
/// - tls: TLS线程本地存储描述符；
/// - ctid: 子线程ID；
///
/// 返回值：成功则返回子进程的线程ID，失败返回-1；
///
/// ```c
/// pid_t ret = syscall(SYS_clone, flags, stack, ptid, tls, ctid)
/// ```

/// 当前进程 fork/clone 出来一个子进程。
///
/// - 参数：
///     - `flags`:
///     - `stack_ptr`
///     - `ptid`
///     - `ctid`
///     - `newtls`
/// - 返回值：对于子进程返回 0，对于当前进程则返回子进程的 PID 。
/// - syscall ID：220
pub fn sys_do_fork(
    flags: usize,
    stack_ptr: usize,
    _ptid: usize,
    _ctid: usize,
    _newtls: usize,
) -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork(false);

    // let tid = new_task.getpid();
    let _flags = CloneFlags::from_bits(flags).unwrap();

    if stack_ptr != 0 {
        let trap_cx = new_task.lock().trap_context();
        trap_cx.set_sp(stack_ptr);
    }
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.lock().trap_context();
    // we do not have to move to next instruction since we have done it before
    // trap_handler 已经将当前进程 Trap 上下文中的 sepc 向后移动了 4 字节，
    // 使得它回到用户态之后，会从发出系统调用的 ecall 指令的下一条指令开始执行

    trap_cx.x[10] = 0; // 对于子进程，返回值是0
    add_task(new_task); // 将 fork 到的进程加入任务调度器
    unsafe {
        core::arch::asm!("sfence.vma");
        core::arch::asm!("fence.i");
    }
    new_pid as isize // 对于父进程，返回值是子进程的 PID
}

/// #define SYS_execve 221
///
/// 功能：执行一个指定的程序；
///
/// 输入：
///
/// - path: 待执行程序路径名称，
/// - argv: 程序的参数，
/// - envp: 环境变量的数组指针
///
/// 返回值：成功不返回，失败返回-1；
///
/// ```c
/// const char *path, char *const argv[], char *const envp[];
/// int ret = syscall(SYS_execve, path, argv, envp);
/// ```
pub fn sys_exec(path: *const u8, mut args: *const usize, mut envs: *const u8) -> isize {
    let token = current_user_token();
    // 读取到用户空间的应用程序名称（路径）
    let path = translated_str(token, path);
    let mut args_vec: Vec<String> = Vec::new();
    if args as usize != 0 {
        loop {
            let arg_str_ptr = *translated_ref(token, args);
            if arg_str_ptr == 0 {
                // 读到下一参数地址为0表示参数结束
                break;
            } // 否则从用户空间取出参数，压入向量
            args_vec.push(translated_str(token, arg_str_ptr as *const u8));
            unsafe {
                args = args.add(1);
            }
        }
    }

    // 环境变量
    let mut envs_vec: Vec<String> = Vec::new();
    if envs as usize != 0 {
        loop {
            let env_str_ptr = *translated_ref(token, envs);
            if env_str_ptr == 0 {
                // 读到下一参数地址为0表示参数结束
                break;
            } // 否则从用户空间取出参数，压入向量
            envs_vec.push(translated_str(token, env_str_ptr as *const u8));
            unsafe {
                envs = envs.add(1);
            }
        }
    }

    let task = current_task().unwrap();

    let inner = task.lock();
    if let Some(app_inode) = open(
        inner.current_path.as_str(),
        path.as_str(),
        OpenFlags::O_RDONLY,
        CreateMode::empty(),
    ) {
        drop(inner);
        task.exec(app_inode, args_vec, envs_vec);

        0 as isize
    } else {
        -1
    }
}