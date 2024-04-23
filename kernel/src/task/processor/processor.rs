use alloc::sync::Arc;
use sync_cell::SyncRefCell;

use crate::task::manager::TASK_MANAGER;
use crate::task::{task::TaskControlBlock, TaskContext};

// lazy_static! {
//     pub static ref PROCESSORS: [SyncRefCell<Processor>; 4] = [
//         SyncRefCell::new(Processor::new()),
//         SyncRefCell::new(Processor::new()),
//         SyncRefCell::new(Processor::new()),
//         SyncRefCell::new(Processor::new()),
//     ];
// }

use spin::lazy::Lazy;
pub static PROCESSORS: Lazy<[SyncRefCell<Processor>; 2]> = Lazy::new(|| {
    [
        SyncRefCell::new(Processor::new()),
        SyncRefCell::new(Processor::new()),
    ]
});

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

impl Processor {
    pub const fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::empty(),
        }
    }
    pub fn idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    /// Take current task.
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }
    pub fn current(&self) -> &Option<Arc<TaskControlBlock>> {
        &self.current
    }
    pub fn current_mut(&mut self) -> &mut Option<Arc<TaskControlBlock>> {
        &mut self.current
    }
    pub fn hang_current(&mut self, sleep_time: usize, duration: usize) {
        TASK_MANAGER
            .lock()
            .hang(sleep_time, duration, self.take_current().unwrap());
    }
}
