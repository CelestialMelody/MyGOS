mod hanging_task;
mod task_manager;

use crate::syscall::impls::FUTEX_QUEUE;

use super::TaskControlBlock;
use alloc::{collections::BTreeMap, sync::Arc};
pub use hanging_task::*;
use spin::Mutex;
pub use task_manager::*;

// lazy_static! {
//     pub static ref TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());
// }

use spin::lazy::Lazy;
pub static TASK_MANAGER: Lazy<Mutex<TaskManager>> = Lazy::new(|| Mutex::new(TaskManager::new()));
pub fn add_task(task: Arc<TaskControlBlock>) {
    PID2TCB.lock().insert(task.pid(), Arc::clone(&task));
    TASK_MANAGER.lock().add(task);
}
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.lock().fetch()
}
pub fn check_hanging() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.lock().check_hanging()
}
pub fn check_futex_interupt_or_expire() -> Option<Arc<TaskControlBlock>> {
    let futex_queue = FUTEX_QUEUE.write();
    TASK_MANAGER
        .lock()
        .check_futex_interupt_or_expire(futex_queue)
}
pub fn unblock_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.lock().unblock_task(task);
}
pub fn block_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.lock().block(task);
}

pub static THREAD_CLEANER: Mutex<CancelledThreads> = Mutex::new(CancelledThreads::new());
pub fn recycle_child_threads_res() {
    THREAD_CLEANER.lock().clear_all();
}
pub fn collect_cancelled_chiled_thread(child_thread: Arc<TaskControlBlock>) {
    THREAD_CLEANER.lock().push(child_thread);
}

// lazy_static! {
//     pub static ref PID2TCB: Mutex<BTreeMap<usize, Arc<TaskControlBlock>>> =
//         Mutex::new(BTreeMap::new());
// }

pub static PID2TCB: Lazy<Mutex<BTreeMap<usize, Arc<TaskControlBlock>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

pub fn pid2task(pid: usize) -> Option<Arc<TaskControlBlock>> {
    let map = PID2TCB.lock();
    map.get(&pid).map(Arc::clone)
}
pub fn remove_from_pid2task(pid: usize) {
    let mut map = PID2TCB.lock();
    if map.remove(&pid).is_none() {
        panic!("cannot find pid {} in pid2task!", pid);
    }
}
