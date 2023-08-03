use super::HangingTask;
use crate::syscall::impls::FUTEX_QUEUE;
use crate::task::TaskControlBlock;
use crate::timer::get_time_ns;
use alloc::collections::{BinaryHeap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;

/// FIFO 任务管理器
pub struct TaskManager {
    // 用于一般进程/线程的调度
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
    // 用于 sleep 的调度, 使用 BinaryHeap 方便对睡眠时间的管理
    hanging_queue: BinaryHeap<HangingTask>,
    // 用于 futex, 唤醒时加入 ready_queue
    waiting_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            waiting_queue: VecDeque::new(),
            hanging_queue: BinaryHeap::new(),
        }
    }
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
    pub fn hang(&mut self, sleep_time: usize, duration: usize, task: Arc<TaskControlBlock>) {
        self.hanging_queue
            .push(HangingTask::new(sleep_time, duration, task));
    }
    pub fn block(&mut self, task: Arc<TaskControlBlock>) {
        self.waiting_queue.push_back(task);
    }
    fn check_sleep(&self, hanging_task: &HangingTask) -> bool {
        let limit = hanging_task.wake_up_time();
        let current_time = get_time_ns();
        current_time >= limit
    }
    pub fn check_hanging(&mut self) -> Option<Arc<TaskControlBlock>> {
        if self.hanging_queue.is_empty() || !self.check_sleep(self.hanging_queue.peek().unwrap()) {
            None
        } else {
            Some(self.hanging_queue.pop().unwrap().inner())
        }
    }
    /// 检查被信号中断的进程/线程中是否有信号处理完成的 或 被 futex_wait 唤醒的
    pub fn check_futex_interupt_or_expire(&mut self) -> Option<Arc<TaskControlBlock>> {
        for tcb in self.waiting_queue.iter() {
            let lock = tcb.inner_ref();
            // { info!("[check_interupt] pid: {:?}, pending_signals: {:?}, sigmask: {:?}", tcb.pid(), lock.pending_signals, lock.sigmask); }
            if !lock.pending_signals.difference(lock.sigmask).is_empty() {
                return Some(tcb.clone());
            }
        }
        let mut global_futex_que = FUTEX_QUEUE.write();
        for (_, futex_queue) in global_futex_que.iter_mut() {
            if let Some(task) = futex_queue.pop_expire_waiter() {
                return Some(task.clone());
            }
        }
        None
    }

    pub fn unblock_task(&mut self, task: Arc<TaskControlBlock>) {
        let p = self
            .waiting_queue
            .iter()
            .enumerate()
            .find(|(_, t)| Arc::ptr_eq(t, &task))
            .map(|(idx, t)| (idx, t.clone()));

        if let Some((idx, task)) = p {
            self.waiting_queue.remove(idx);
            self.add(task);
        }
    }
}

/// 用于线程退出时对线程的收集, 与进程/线程调度时清理
pub struct CancelledThreads {
    collector: Vec<Arc<TaskControlBlock>>,
}

impl CancelledThreads {
    pub const fn new() -> Self {
        CancelledThreads {
            collector: Vec::new(),
        }
    }
    pub fn push(&mut self, child_thread: Arc<TaskControlBlock>) {
        self.collector.push(child_thread);
    }
    pub fn clear_all(&mut self) {
        self.collector.clear();
    }
}