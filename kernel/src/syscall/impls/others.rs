//! About syscall detail: https://man7.org/linux/man-pages/dir_section_2.html

use nix::info::Utsname;
use nix::{itimerval, tms, IntervalTimer, IntervalTimerType, TimeSpec, TimeVal};

use crate::mm::translated_mut;
use crate::return_errno;
use crate::task::{current_task, hanging_current_and_run_next};
use crate::timer::get_time_ns;
use crate::{
    mm::{translated_bytes_buffer, translated_ref, UserBuffer},
    task::{current_user_token, suspend_current_and_run_next},
    timer::{get_time_ms, get_timeval},
};

use super::*;

// times 153
pub fn sys_times(buf: *const u8) -> Result {
    let sec = get_time_ms() as isize * 1000;
    let token = current_user_token();
    let buffers = translated_bytes_buffer(token, buf, core::mem::size_of::<tms>());
    let mut userbuf = UserBuffer::wrap(buffers);
    // TODO tms rusage
    userbuf.write(
        tms {
            tms_stime: sec,
            tms_utime: sec,
            tms_cstime: sec,
            tms_cutime: sec,
        }
        .as_bytes(),
    );
    Ok(0)
}

// uname 160
pub fn sys_uname(buf: *const u8) -> Result {
    let token = current_user_token();
    let mut userbuf = UserBuffer::wrap(translated_bytes_buffer(
        token,
        buf,
        core::mem::size_of::<Utsname>(),
    ));
    userbuf.write(Utsname::get().as_bytes());
    Ok(0)
}

// sched_yield 124
pub fn sys_sched_yield() -> Result {
    suspend_current_and_run_next();
    Ok(0)
}

// gettimeofday 169
pub fn sys_gettimeofday(buf: *const u8) -> Result {
    let token = current_user_token();
    let buffers = translated_bytes_buffer(token, buf, core::mem::size_of::<TimeVal>());
    let mut userbuf = UserBuffer::wrap(buffers);
    userbuf.write(get_timeval().as_bytes());
    Ok(0)
}

// nanosleep 101
pub fn sys_nanosleep(buf: *const u8) -> Result {
    let tic = get_time_ns();
    let token = current_user_token();
    let res = translated_ref(token, buf as *const TimeSpec);
    let len = res.into_ns();
    hanging_current_and_run_next(tic, len);
    Ok(0)
}

// clock_nanosleep 115
pub fn sys_clock_nanosleep(
    _clock_id: usize,
    flags: isize,
    req: *const TimeSpec,
    _remain: *mut TimeSpec,
) -> Result {
    if flags == 1 {
        // TIMER_ABSTIME
        let current_time = get_time_ns();
        let token = current_user_token();
        let res = translated_ref(token, req as *const TimeSpec);
        let abs_time = res.into_ns();
        // assert!(abs_time >= current_time);
        if abs_time > current_time {
            let interval = abs_time - current_time;
            hanging_current_and_run_next(current_time, interval);
        }
        Ok(0)
    } else {
        sys_nanosleep(req as *const u8)
    }
}

// getrandom 278
// TODO
pub fn sys_getrandom(_buf: *const u8, buf_size: usize, _flags: usize) -> Result {
    Ok(buf_size as isize)
}

// setitimer 103
pub fn sys_setitimer(which: i32, new_value: *const itimerval, old_value: *mut itimerval) -> Result {
    let nvp_usize = new_value as usize;
    let ovp_usize = old_value as usize;
    if nvp_usize == 0 {
        return_errno!(Errno::EFAULT);
    }
    if let Ok(itimer_type) = IntervalTimerType::try_from(which) {
        let task = current_task().unwrap();
        if ovp_usize != 0 {
            let inner = task.inner_ref();
            if let Some(itimer) = &inner.interval_timer {
                let ov = translated_mut(task.token(), old_value);
                *ov = itimer.timer_value;
            }
        }
        match itimer_type {
            IntervalTimerType::Real => {
                // 是否删除当前 itmer/新设置的 itmer 是否只触发一次
                let nv = translated_ref(task.token(), new_value);
                let zero = TimeVal::zero();
                let mut inner = task.inner_mut();
                if nv.it_interval == zero && nv.it_value == zero {
                    inner.interval_timer = None;
                    return Ok(0);
                }
                inner.interval_timer = Some(IntervalTimer::new(*nv, get_timeval()));
            }
            // TODO: 用到再写
            IntervalTimerType::Virtual => {
                unimplemented!("ITIMER_VIRTUAL")
            }
            IntervalTimerType::Profile => {
                unimplemented!("ITIMER_PROF")
            }
        }
    } else {
        return_errno!(
            Errno::EINVAL,
            "which {} is not one of ITIMER_REAL, ITIMER_VIRTUAL, or ITIMER_PROF",
            which
        );
    }
    Ok(0)
}

// getitimer 102
pub fn sys_getitimer(which: i32, curr_value: *mut itimerval) -> Result {
    let cv_usize = curr_value as usize;
    if cv_usize == 0 {
        return_errno!(Errno::EFAULT);
    }
    if let Ok(itimer_type) = IntervalTimerType::try_from(which) {
        let task = current_task().unwrap();
        match itimer_type {
            IntervalTimerType::Real => {
                let inner = task.inner_ref();
                let cv = translated_mut(task.token(), curr_value);
                *cv = if let Some(itimerval) = &inner.interval_timer {
                    itimerval.timer_value
                } else {
                    itimerval::empty()
                };
            }
            // TODO: 用到再写
            IntervalTimerType::Virtual => {
                unimplemented!("ITIMER_VIRTUAL")
            }
            IntervalTimerType::Profile => {
                unimplemented!("ITIMER_PROF")
            }
        }
    } else {
        return_errno!(
            Errno::EINVAL,
            "which {} is not one of ITIMER_REAL, ITIMER_VIRTUAL, or ITIMER_PROF",
            which
        );
    }
    Ok(0)
}

// timer_settime 110
pub fn sys_timer_settime(
    _time_id: usize,
    _flags: isize,
    new_value: *const itimerval,
    old_value: *mut itimerval,
) -> Result {
    let task = current_task().unwrap();
    if new_value as usize != 0 {
        let nv = translated_ref(task.token(), new_value);
        let zero = TimeVal::zero();
        let mut inner = task.inner_mut();
        if nv.it_interval == zero && nv.it_value == zero {
            inner.interval_timer = None;
        }
        inner.interval_timer = Some(IntervalTimer::new(*nv, get_timeval()));
    }
    if old_value as usize != 0 {
        let inner = task.inner_ref();
        if let Some(itimer) = &inner.interval_timer {
            let ov = translated_mut(task.token(), old_value);
            *ov = itimer.timer_value;
        }
    }
    Ok(0)
}

// timer_getoverrun 109
pub fn sys_timer_getoverrun(_timerid: usize) -> Result {
    Ok(0)
}

// recvfrom 207
pub fn sys_recvfrom(
    _sockfd: usize,
    buf: *mut u8,
    _len: usize,
    _flags: usize,
    _src_addr: usize,
    _addrlen: usize,
) -> Result {
    let src = "x";
    let token = current_user_token();
    let len = src.as_bytes().len();
    let mut buffer = UserBuffer::wrap(translated_bytes_buffer(token, buf, len));
    buffer.write(src.as_bytes());
    Ok(1)
}
