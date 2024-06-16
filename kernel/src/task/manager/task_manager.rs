use super::HangingTask;
use crate::syscall::futex::FutexQueue;
use crate::task::TaskControlBlock;
use crate::timer::get_time_ns;
use alloc::collections::{BinaryHeap, VecDeque};
use alloc::sync::Arc;
use alloc::vec::Vec;
use hashbrown::HashMap;
use spin::RwLockWriteGuard;

/// FIFO Task Manager
pub struct TaskManager {
    // Used for general process/thread scheduling
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
    // Used for sleep scheduling, using BinaryHeap to facilitate the management of sleep time
    hanging_queue: BinaryHeap<HangingTask>,
    // Used for futex, add to ready_queue when waking up
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
    /// Check if there are signals that have been processed in the process/thread interrupted by the signal or woken up by futex_wait
    pub fn check_futex_interupt_or_expire(
        &mut self,
        mut global_futex_queue: RwLockWriteGuard<HashMap<usize, FutexQueue>>,
    ) -> Option<Arc<TaskControlBlock>> {
        for tcb in self.waiting_queue.iter() {
            let lock = tcb.inner_ref();
            // { info!("[check_interupt] pid: {:?}, pending_signals: {:?}, sigmask: {:?}", tcb.pid(), lock.pending_signals, lock.sigmask); }
            if !lock.pending_signals.difference(lock.sigmask).is_empty() {
                return Some(tcb.clone());
            }
        }
        for (_, futex_queue) in global_futex_queue.iter_mut() {
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

/// Used to collect threads when threads exit, and clean up when process/thread scheduling
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
