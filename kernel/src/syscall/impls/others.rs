use nix::info::Utsname;
use nix::{
    itimerval, tms, IntervalTimer, IntervalTimerType, TimeSpec, TimeVal, ITIMER_PROF, ITIMER_REAL,
    ITIMER_VIRTUAL,
};

use crate::mm::translated_mut;
use crate::return_errno;
use crate::task::{current_task, hanging_current_and_run_next};
use crate::timer::{get_time, get_time_ns};
use crate::{
    mm::{translated_bytes_buffer, translated_ref, UserBuffer},
    task::{current_user_token, suspend_current_and_run_next},
    timer::{get_time_ms, get_timeval},
};

use super::*;

/// #define SYS_times 153
///
/// 功能: 获取进程时间;
///
/// 输入: tms结构体指针, 用于获取保存当前进程的运行时间数据;
///
/// 返回值: 成功返回已经过去的滴答数, 失败返回-1;
///
/// ```c
/// struct tms *tms;
/// clock_t ret = syscall(SYS_times, tms);
/// ```
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

// TODO 2 tms 没有处理

/// struct utsname {
/// 	char sysname\[65\];
/// 	char nodename\[65\];
/// 	char release\[65\];
/// 	char version\[65\];
/// 	char machine\[65\];
/// 	char domainname\[65\];
/// };
///
/// #define SYS_uname 160
///
/// 功能: 打印系统信息;
///
/// 输入: utsname结构体指针用于获得系统信息数据;
///
/// 返回值: 成功返回0, 失败返回-1;
///
/// ```c
/// struct utsname *uts;
/// int ret = syscall(SYS_uname, uts);
/// ```
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

/// 应用主动交出 CPU 所有权进入 Ready 状态并切换到其他应用
///
/// - 返回值: 总是返回 0.
/// - syscall ID: 124
pub fn sys_sched_yield() -> Result {
    suspend_current_and_run_next();
    Ok(0)
}

// TODO 1 规范里面写的 timeval

/// ```c
/// struct timespec {
/// 	time_t tv_sec;        /* 秒 */
/// 	long   tv_nsec;       /* 纳秒, 范围在0~999999999 */
/// };
///
/// ```
///
/// #define SYS_gettimeofday 169
///
/// 功能: 获取时间;
///
/// 输入:  timespec结构体指针用于获得时间值;
///
/// 返回值: 成功返回0, 失败返回-1;
///
/// ```c
/// struct timespec *ts;
/// int ret = syscall(SYS_gettimeofday, ts, 0);
/// ```
pub fn sys_gettimeofday(buf: *const u8) -> Result {
    let token = current_user_token();
    let buffers = translated_bytes_buffer(token, buf, core::mem::size_of::<TimeVal>());
    let mut userbuf = UserBuffer::wrap(buffers);
    userbuf.write(get_timeval().as_bytes());
    Ok(0)
}

/// #define SYS_nanosleep 101
///
/// 功能: 执行线程睡眠, sleep()库函数基于此系统调用;
///
/// 输入: 睡眠的时间间隔;
///
/// 返回值: 成功返回0, 失败返回-1;
///
/// ```c
/// const struct timespec *req, struct timespec *rem;
/// int ret = syscall(SYS_nanosleep, req, rem);
/// ```
// TODO未按要求实现
pub fn sys_nanosleep(buf: *const u8) -> Result {
    let tic = get_time_ns();
    let token = current_user_token();
    let res = translated_ref(token, buf as *const TimeSpec);
    let len = res.into_ns();
    hanging_current_and_run_next(tic, len);
    Ok(0)
}
// int clock_nanosleep(clockid_t clockid, int flags,
//      const struct timespec *request,
//      struct timespec *_Nullable remain);
pub fn sys_clock_nanosleep(
    clock_id: usize,
    flags: isize,
    req: *const TimeSpec,
    _remain: *mut TimeSpec,
) -> Result {
    if (flags == 1) {
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

pub fn sys_getrandom(buf: *const u8, buf_size: usize, flags: usize) -> Result {
    Ok(buf_size as isize)
}

pub fn sys_setitimer(which: i32, new_value: *const itimerval, old_value: *mut itimerval) -> Result {
    let nvp_usize = new_value as usize;
    let ovp_usize = old_value as usize;
    if nvp_usize == 0 {
        return_errno!(Errno::EFAULT);
    }
    if let Ok(itimer_type) = IntervalTimerType::try_from(which) {
        let task = current_task();
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

pub fn sys_getitimer(which: i32, curr_value: *mut itimerval) -> Result {
    let cv_usize = curr_value as usize;
    if cv_usize == 0 {
        return_errno!(Errno::EFAULT);
    }
    if let Ok(itimer_type) = IntervalTimerType::try_from(which) {
        let task = current_task();
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

// TODO timerid 指定的计时器
pub fn sys_timer_settime(
    _time_id: usize,
    _flags: isize,
    new_value: *const itimerval,
    old_value: *mut itimerval,
) -> Result {
    let task = current_task();
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

pub fn sys_timer_getoverrun(_timerid: usize) -> Result {
    Ok(0)
}

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
