## 进程/线程调度

在我们的内核中，我们使用 TASKMANAGER 管理分别处于就绪态，阻塞态的进程，包括因为调用 nanosleep 而休眠的进程。

```rust
// kernel/src/task/manager.rs

// 负责管理待调度的进程对象
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
    waiting_queue: VecDeque<Arc<TaskControlBlock>>,
    hq: BinaryHeap<HangingTask>,
}

// 用于管理 sleep 进程
pub struct HangingTask {
    wake_up_time: usize, // ns
    inner: Arc<TaskControlBlock>,
}

// 用于处理子线程的资源回收
pub struct ChildrenThreadMonitor {
    cancelled_child_threads: Vec<Arc<TaskControlBlock>>,
}

// 维护内核中 pid 到 TCB 的映射
pub static ref PID2TCB: Mutex<BTreeMap<usize, Arc<TaskControlBlock>>> =
    Mutex::new(BTreeMap::new());

// 子线程回收管理器
pub static CHILDREN_THREAD_MONITOR: Mutex<ChildrenThreadMonitor> =
    Mutex::new(ChildrenThreadMonitor::new());
// kernel/src/task/processor/processor.rs

/// [`Processor`] 是描述 CPU执行状态 的数据结构。
/// 在单核环境下，我们仅创建单个 Processor 的全局实例 PROCESSOR
pub static mut PROCESSOR: SyncRefCell<Processor> = SyncRefCell::new(Processor::new());

/// 每个核上的处理器，负责运行一个进程
pub struct Processor {
    /// 当前处理器上正在执行的任务
    current: Option<Arc<TaskControlBlock>>,
    /// 当前处理器上的 idle 控制流的任务上下文
    idle_task_cx: TaskContext,
}
```

`run_tasks` 分别尝试从 hang_task, ready_task 队列中获取进程调度。

```rust
// kernel/src/task/processor/schedule.rs

/// 进入 idle 控制流，它运行在这个 CPU 核的启动栈上，
/// 功能是循环调用 fetch_task 直到顺利从任务管理器中取出一个任务，随后便准备通过任务切换的方式来执行
pub fn run_tasks() {
    let bb = BUSYBOX.read(); // lazy static busybox
    drop(bb);
    loop {
        let processor = acquire_processor();

        recycle_child_threads_res();

        if let Some(hanging_task) = check_hanging() {
            run_task(hanging_task, processor);
        } else if let Some(interupt_task) = check_futex_interupt_or_expire() {
            unblock_task(interupt_task);
        } else if let Some(task) = fetch_task() {
            run_task(task, processor);
        }
    }
}
```

值得一提的是，我们在进程调度时还需要检测 block_task 队列中，因为在系统调用过程中被信号打断的 task 是否有处理完信号，或者 futex_wait 时给出的 timeout 是否已超时以唤醒该进程并加入到 ready_task 中。

```rust
// kernel/src/task/manager.rs

pub fn check_futex_interupt_or_expire(&mut self) -> Option<Arc<TaskControlBlock>> {
        for tcb in self.waiting_queue.iter() {
            let lock = tcb.inner_ref();
            // 被信号打断的 task 是否有处理完信号
            if !lock.pending_signals.difference(lock.sigmask).is_empty() {
                return Some(tcb.clone());
            }
        }
        let mut global_futex_que = FUTEX_QUEUE.write();
        for (_, futex_queue) in global_futex_que.iter_mut() {
            // timeout 是否已超时
            if let Some(task) = futex_queue.pop_expire_waiter() {
                return Some(task.clone());
            }
        }
        None
    }
```



