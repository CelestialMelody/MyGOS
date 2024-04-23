#![allow(unused)]

use crate::{
    boards::CLOCK_FREQ,
    sbi::set_timer,
    task::{current_add_signal, current_task, TaskControlBlock},
};
use alloc::sync::Arc;
use core::ops::{Add, Sub};
use nix::{SigMask, TimeVal};
use riscv::register::time;

pub const TIME_SLICE: usize = 100;
pub const MSEC_PER_SEC: usize = 1000;
pub const USEC_PER_SEC: usize = 1000_000;
pub const NSEC_PER_SEC: usize = 1000_000_000;

/// 取得当前 `mtime` 计数器的值
///
/// - `mtime`: 统计处理器自上电以来经过了多少个内置时钟的时钟周期,64bit
pub fn get_time() -> usize {
    time::read()
}

/// 获取CPU上电时间(单位: ms)
pub fn get_time_ms() -> usize {
    get_time() / (CLOCK_FREQ / MSEC_PER_SEC)
}

pub fn get_time_ns() -> usize {
    (get_time() / (CLOCK_FREQ / USEC_PER_SEC)) * MSEC_PER_SEC
}

pub fn get_time_us() -> usize {
    get_time() / (CLOCK_FREQ / USEC_PER_SEC)
}

pub fn get_time_s() -> usize {
    get_time() / CLOCK_FREQ
}

/// 获取 `TimeVal` 格式的时间信息
pub fn get_timeval() -> TimeVal {
    let ticks = get_time();
    let sec = ticks / CLOCK_FREQ;
    let usec = (ticks % CLOCK_FREQ) * USEC_PER_SEC / CLOCK_FREQ;
    TimeVal { sec, usec }
}

/// Setup the next time interrupt time.
pub fn set_next_trigger() {
    set_timer((get_time() + CLOCK_FREQ / TIME_SLICE) as u64);
}

pub fn check_interval_timer() {
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    if inner.interval_timer.is_none() {
        return;
    }

    let itimer = inner.interval_timer.as_mut().unwrap();
    let duration = get_timeval() - itimer.creation_time;
    let itime_value = itimer.timer_value;
    // INFO: 现在只处理 ITIMER_REAL
    if duration > itime_value.it_value {
        if itime_value.it_interval == TimeVal::zero() {
            inner.interval_timer = None;
        } else {
            itimer.timer_value.it_value = itimer.timer_value.it_interval;
        }
        inner.pending_signals.set(SigMask::SIGALRM, true);
    }
}
