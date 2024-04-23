## TCB 与 fork 的更改

相较于初赛，完善了 clone 系统调用。初赛时由于要求测试样例要求比较低，在实现 clone 系统调用时并未完全利用用户传递的参数。我们根据 Linux manual page 中的要求，完善了内核的 fork 以及 TaskControlBlock 结构。

<!--more-->

```rust
// kernel/task/task.rs

pub struct TaskControlBlock {
	...
    pub sigactions: Arc<RwLock<[SigAction; MAX_SIGNUM as usize]>>,
    pub memory_set: Arc<RwLock<MemorySet>>,
    pub fd_table: Arc<RwLock<FDTable>>,
    pub robust_list: Arc<RwLock<RobustList>>,
    pub rlimit_nofile: Arc<RwLock<RLimit>>,
    inner: RwLock<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
	...
    pub pending_signals: SigSet,
    pub sigmask: SigMask,
    pub interval_timer: Option<IntervalTimer>,
    pub utime: TimeVal,
    pub stime: TimeVal,
    pub last_enter_umode_time: TimeVal,
    pub last_enter_smode_time: TimeVal,
    pub clear_child_tid: usize, /* CLONE_CHILD_CLEARTID */
}
```

相较于初赛，我们为 TCB 加入了有关信号、时间、资源等结构。并根据 sys_clone 传递的参数，正确地实现 fork，比如以下代码段：

```rust
// kernel/src/task/task.rs(fn fork)

// 拷贝用户地址空间
let memory_set = if flags.contains(CloneFlags::VM) {
    self.memory_set.clone()
} else {
    Arc::new(RwLock::new(MemorySet::from_copy_on_write(
        &mut self.memory_set.write(),
    )))
};
if flags.contains(CloneFlags::THREAD) {
    memory_set.write().map_thread_trap_context(private_tid);
}
```

```rust
// kernel/syscall/impls/process.rs (sys_do_fork)

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
```



### 线程的引入

在 fork 过程中，当 CloneFlags 中存在 CLONE_THREAD 位时，正在创建的进程当前进程的为子线程

**Pid 分配器的更改**

由于我们的内核在实现线程时，为了更方便地为子线程分配 TrapContext Frame 资源，我们规定子线程的 tid (pid) 不应该与小于进程（主线程）的 tid (pid) ，故移除了进程 pid 的回收操作。

**子进程与子线程的区分**

目前我们将子线程与子进程均保存在 TCB 的 children 字段，在遇到进程退出等问题时会判断 child 是子进程还是子线程。

目前通过 tgid 与 pid 来区分 TCB 是父进程的子进程还是子线程

```rust
// kernel/src/task/task.rs(fn fork)

let pid_handle = pid_alloc();
let tgid = if flags.contains(CloneFlags::THREAD) {
    self.pid.0
} else {
    pid_handle.0
};
```

如果 tgid 与 pid 值相同，则该 TCB 为进程，否则为线程

**为线程分配资源**

线程除了共享主线程（进程）的 memory_set, fd_table, sigaction 等资源，还需要一些独立的资源如ID, 内核地址空间的KernelStack，以及主线程中独立分配的 TrapContext Frame:

```rust
// kernel/syscall/impls/process.rs (sys_do_fork)

if flags.contains(CloneFlags::THREAD) {
    memory_set.write().map_thread_trap_context(private_tid);
}

// kernel/src/task/ttask.rs
pub fn trap_context_position(tid: usize) -> VirtAddr {
    VirtAddr::from(TRAP_CONTEXT - tid * PAGE_SIZE)
}
```

其中，private_tid 为tgid(主线程/父进程)与pid(子线程tid)的差值



### 进程/线程的退出

当前进程结束的方式包括：

1. 进程运行完代码段，非法访问到 .rodata 引发 trap，在 trap 处理中回收进程对象
2. 进程调用 exit 系统调用
3. 进程收到 kill 相关的信号，在信号处理时退出

关于进程/线程的退出时需要做的工作包括：

1. 完成进程的初步回收：
   1. 将自身从 PID2TCB 映射管理器中移除
   2. 标记自身状态为 Zombie，记录退出码
   3. 将子进程移交给 initproc
   4. 如果自身为某进程的子线程，还需要找到主进程并将自身从主进程中移除，并压入子线程回收管理器 CHILDREN_THREAD_MONITOR 中，在*下一次进程调度时回收可回收的资源*
2. 进程的父进程等待子进程退出，调用 wait 系统调用，完成子进程资源的回收：找到子进程中处于 Zombie 态的进程并且强引用计数为 1 的进程，移除该进程以彻底回收该进程的所有资源

**子线程资源回收**

子线程退出时，子线程会加入到回收管理器 CHILDREN_THREAD_MONITOR 中，并在*下一次进程调度时回收可回收的资源*。

```rust
// kernel/src/task/mod.rs (fn exit_current_and_run_next)

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
    take_cancelled_chiled_thread(task);
    schedule(&mut TaskContext::empty() as *mut _);
    unreachable!()
}
```

这个过程本身其实可以不用做，而是等主线程进行 wait 系统调用时彻底回收。但由于测试过程中，进程会创建成千上万个子线程，如果这些线程资源没有及时回收，如 TrapContext, KenerlStack 等资源，会浪费许多内存资源。

其实 `take_cancelled_chiled_thread(task)`这段代码本身，以及 `CHILDREN_THREAD_MONITOR`变量，也就是说这段代码本身其实可以直接改为 `drop(task)`，因为此时 task 强引用计数一定为 1，task 中可以释放的资源都可以在, `schedule` 之前释放掉但是 task 在执行 `exit_current_and_run_next`时本身出于内核态，此时回收 task 的 KerenlStack 既不符合逻辑，又有可能产生一些隐患，故选择使用 `CHILDREN_THREAD_MONITOR` 在调度时释放退出的线程。