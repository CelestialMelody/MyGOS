#![no_std]
extern crate alloc;
extern crate spin;
// #[macro_use]
// extern crate lazy_static;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use riscv::register::time;
use spin::Lazy;
use spin::Mutex;

pub fn get_time() -> usize {
    time::read()
}
/// 获取CPU上电时间(单位: ms)
pub const CLOCK_FREQ: usize = 12500000;
pub const TIME_SLICE: usize = 100;
pub const MSEC_PER_SEC: usize = 1000;
pub const USEC_PER_SEC: usize = 1000_000;
pub const NSEC_PER_SEC: usize = 1000_000_000;

pub fn get_time_ns() -> usize {
    (get_time() / (CLOCK_FREQ / USEC_PER_SEC)) * MSEC_PER_SEC
}

// lazy_static! {
//     pub static ref TIME_ALL: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());
//     pub static ref TIME_STACK: Mutex<Vec<TimeTracer>> = Mutex::new(Vec::new());
// }

// TODO
pub static TIME_ALL: Lazy<Mutex<BTreeMap<String, usize>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
pub static TIME_STACK: Lazy<Mutex<Vec<TimeTracer>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub struct TimeTracer {
    tag: String,
    time: usize,
}
impl TimeTracer {
    pub fn new(tag: String, time: usize) -> Self {
        Self { tag, time }
    }
}
impl Drop for TimeTracer {
    fn drop(&mut self) {
        let mut lock = TIME_ALL.lock();
        let new_time = get_time_ns() - self.time;
        let old_time = if lock.get(&self.tag).is_none() {
            0 as usize
        } else {
            *lock.get(&self.tag).unwrap()
        };
        lock.insert(self.tag.clone(), new_time + old_time);
    }
}

#[macro_export]
macro_rules! time_trace {
    ($msg: literal) => {
        let s = alloc::string::String::from($msg);
        let time_tracer = $crate::TimeTracer::new(_s, $crate::get_time_ns());
    };
}

#[macro_export]
macro_rules! start_trace {
    ($msg: literal) => {
        let s = alloc::string::String::from($msg);
        let time_tracer = $crate::TimeTracer::new(_s, $crate::get_time_ns());
        let mut lock = $crate::TIME_STACK.lock();
        lock.push(time_tracer);
        drop(lock);
    };
}

#[macro_export]
macro_rules! end_trace {
    () => {
        let mut lock = $crate::TIME_STACK.lock();
        lock.pop();
        drop(lock);
    };
}
