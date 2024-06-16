use super::impls::*;
use super::*;
use nix::{itimerval, time::TimeSpec};
use nix::{RLimit, SchedParam, SigAction, SigMask};

/// Syscall dispatcher.
pub fn syscall(id: usize, args: [usize; 6]) -> isize {
    let syscall_id = SyscallId::from(id); // This will check if syscall id is valid.

    // if syscall_id != SyscallId::SYS_BRK {
    //     println!("On hart {}: [{}]", hartid!(), syscall_id);
    // }
    let ret = match syscall_id {
        SyscallId::SYS_CLONE => sys_do_fork(args[0], args[1], args[2], args[3], args[4]),

        SyscallId::SYS_TGKILL => sys_tgkill(args[0] as isize, args[1], args[2] as isize),

        SyscallId::SYS_EXECVE => sys_exec(
            args[0] as *const u8,
            args[1] as *const usize,
            args[2] as *const usize,
        ),

        SyscallId::SYS_LINKAT => sys_linkat(
            args[0] as isize,
            args[1] as *const u8,
            args[2] as isize,
            args[3] as *const u8,
            args[4] as u32,
        ),

        SyscallId::SYS_OPENAT => sys_openat(
            args[0] as i32,
            args[1] as *const u8,
            args[2] as u32,
            args[3] as u32,
        ),

        SyscallId::SYS_GETCWD => sys_getcwd(args[0] as *mut u8, args[1]),
        SyscallId::SYS_DUP => sys_dup(args[0]),
        SyscallId::SYS_DUP3 => sys_dup3(args[0], args[1]),
        SyscallId::SYS_MKDIRAT => sys_mkdirat(args[0] as i32, args[1] as *const u8, args[2] as u32),
        SyscallId::SYS_UNLINKAT => {
            sys_unlinkat(args[0] as isize, args[1] as *const u8, args[2] as u32)
        }
        SyscallId::SYS_UMOUNT2 => sys_umount2(args[0] as *const u8, args[1]),
        SyscallId::SYS_MOUNT => sys_mount(
            args[0] as *const u8,
            args[1] as *const u8,
            args[2] as *const u8,
            args[3],
            args[4] as *const u8,
        ),
        SyscallId::SYS_CHDIR => sys_chdir(args[0] as *const u8),
        SyscallId::SYS_CLOSE => sys_close(args[0]),
        SyscallId::SYS_PIPE2 => sys_pipe2(args[0] as *mut i32, args[1] as i32),
        SyscallId::SYS_GETDENTS64 => sys_getdents64(args[0] as isize, args[1] as *mut u8, args[2]),
        SyscallId::SYS_READ => sys_read(args[0], args[1] as *const u8, args[2]),
        SyscallId::SYS_WRITE => sys_write(args[0] as i32, args[1] as *const u8, args[2]),
        SyscallId::SYS_FSTAT => sys_fstat(args[0] as i32, args[1] as *mut u8),
        SyscallId::SYS_EXIT => sys_exit(args[0] as i32),
        SyscallId::SYS_NANOSLEEP => sys_nanosleep(args[0] as *const u8),
        SyscallId::SYS_SCHED_YIELD => sys_sched_yield(),
        SyscallId::SYS_TIMES => sys_times(args[0] as *const u8),
        SyscallId::SYS_UNAME => sys_uname(args[0] as *const u8),
        SyscallId::SYS_GETTIMEOFDAY => sys_gettimeofday(args[0] as *const u8),
        SyscallId::SYS_GETPID => sys_getpid(),
        SyscallId::SYS_GETPPID => sys_getppid(),
        SyscallId::SYS_BRK => sys_brk(args[0]),
        SyscallId::SYS_MMAP => sys_mmap(
            args[0],
            args[1],
            args[2],
            args[3],
            args[4] as isize,
            args[5],
        ),
        SyscallId::SYS_MUNMAP => sys_munmap(args[0], args[1]),
        SyscallId::SYS_WAIT4 => sys_wait4(args[0] as isize, args[1] as *mut i32),
        SyscallId::SYS_SET_TID_ADDRESS => sys_set_tid_address(args[0] as *mut usize),
        SyscallId::SYS_READV => sys_readv(args[0], args[1] as *const usize, args[2]),
        SyscallId::SYS_WRITEV => sys_writev(args[0], args[1] as *const usize, args[2]),
        SyscallId::SYS_EXIT_GROUP => sys_exit_group(args[0] as i32),
        SyscallId::SYS_GETUID => sys_getuid(),
        SyscallId::SYS_IOCTL => sys_ioctl(args[0] as i32, args[1], args[2] as *mut u8),
        SyscallId::SYS_FCNTL => sys_fcntl(
            args[0] as i32,
            args[1] as usize,
            Option::<usize>::from(args[2]),
        ),
        SyscallId::SYS_GETEUID => sys_geteuid(),
        SyscallId::SYS_PPOLL => sys_ppoll(
            args[0],
            args[1],
            args[2] as *const TimeSpec,
            args[3] as *const SigMask,
        ),
        SyscallId::SYS_NEWFSTATAT => sys_newfstatat(
            args[0] as isize,
            args[1] as *const u8,
            args[2] as *const usize,
            args[3],
        ),
        SyscallId::SYS_CLOCK_GETTIME => sys_clock_gettime(args[0], args[1] as *mut u64),
        SyscallId::SYS_GETTID => sys_gettid(),
        SyscallId::SYS_SENDFILE => sys_sendfile(args[0] as i32, args[1] as i32, args[2], args[3]),
        SyscallId::SYS_SYSLOG => Ok(0),
        SyscallId::SYS_FACCESSAT => Ok(0),
        SyscallId::SYS_SYSINFO => Ok(0),
        SyscallId::SYS_KILL => sys_kill(args[0], args[1] as u32),
        SyscallId::SYS_UTIMENSAT => sys_utimensat(
            args[0] as isize,
            args[1] as *const u8,
            args[2] as *const [TimeSpec; 2],
            args[3],
        ),
        SyscallId::SYS_RENAMEAT2 => sys_renameat2(
            args[0] as isize,
            args[1] as *const u8,
            args[2] as isize,
            args[3] as *const u8,
            args[4] as u32,
        ),
        SyscallId::SYS_LSEEK => sys_lseek(args[0], args[1] as isize, args[2]),
        SyscallId::SYS_GETEGID => Ok(0),
        SyscallId::SYS_GETGID => Ok(0),
        SyscallId::SYS_SET_ROBUST_LIST => sys_set_robust_list(args[0], args[1]),
        SyscallId::SYS_GET_ROBUST_LIST => {
            sys_get_robust_list(args[0], args[1] as *mut usize, args[2] as *mut usize)
        }
        SyscallId::SYS_PRLIMIT64 => sys_prlimit64(
            args[0],
            args[1] as u32,
            args[2] as *const RLimit,
            args[3] as *mut RLimit,
        ),
        SyscallId::SYS_READLINKAT => sys_readlinkat(
            args[0] as isize,
            args[1] as *const u8,
            args[2] as *const u8,
            args[3],
        ),
        SyscallId::SYS_GETRANDOM => sys_getrandom(args[0] as *const u8, args[1], args[2]),
        SyscallId::SYS_GETPGID => Ok(0),
        SyscallId::SYS_SETPGID => Ok(0),
        SyscallId::SYS_SYNC => sys_sync(),
        SyscallId::SYS_FTRUNCATE64 => sys_ftruncate64(args[0], args[1]),
        SyscallId::SYS_PSELECT6 => sys_pselect6(
            args[0] as usize,
            args[1] as *mut u8,
            args[2] as *mut u8,
            args[3] as *mut u8,
            args[4] as *mut usize,
        ),
        SyscallId::SYS_GETRUSAGE => sys_getrusage(args[0] as isize, args[1] as *mut u8),
        SyscallId::SYS_SETITIMER => sys_setitimer(
            args[0] as i32,
            args[1] as *const itimerval,
            args[2] as *mut itimerval,
        ),
        SyscallId::SYS_GETITIMER => sys_getitimer(args[0] as i32, args[1] as *mut itimerval),
        SyscallId::SYS_UMASK => Ok(0),
        SyscallId::SYS_FSYNC => Ok(0),
        SyscallId::SYS_MSYNC => Ok(0),
        SyscallId::SYS_SHMGET => sys_shmget(args[0], args[1], args[2]),
        SyscallId::SYS_SHMCTL => sys_shmctl(args[0], args[1], args[2] as *const u8),
        SyscallId::SYS_SHMAT => sys_shmat(args[0], args[1], args[2]),
        SyscallId::SYS_SHMDT => sys_shmdt(args[0]),
        SyscallId::SYS_PREAD64 => sys_pread64(args[0], args[1] as *const u8, args[2], args[3]),
        SyscallId::SYS_PWRITE64 => {
            sys_pwrite64(args[0] as i32, args[1] as *const u8, args[2], args[3])
        }
        SyscallId::SYS_STATFS => sys_statfs(args[0] as *const u8, args[1] as *const u8),
        SyscallId::SYS_SIGTIMEDWAIT => Ok(0),
        SyscallId::SYS_MPROTECT => sys_mprotect(args[0], args[1], args[2]),
        SyscallId::SYS_MEMBARRIER => Ok(0),
        SyscallId::SYS_SCHED_GETAFFINITY => {
            sys_sched_getaffinity(args[0] as usize, args[1] as usize, args[2] as *mut u8)
        }
        SyscallId::SYS_SCHEED_GETSCHEDULER => sys_getscheduler(args[0] as usize),
        SyscallId::SYS_SCHED_GETPARAM => {
            sys_sched_getparam(args[0] as usize, args[1] as *mut SchedParam)
        }
        SyscallId::SYS_SCHED_SETSCHEDULER => sys_sched_setscheduler(
            args[0] as usize,
            args[1] as isize,
            args[2] as *const SchedParam,
        ),
        SyscallId::SYS_CLOCK_GETRES => sys_clock_getres(args[0] as usize, args[1] as *mut TimeSpec),
        SyscallId::SYS_SOCKETPAIR => sys_socketpair(
            args[0] as isize,
            args[1] as isize,
            args[2] as isize,
            args[3] as *mut [i32; 2],
        ),
        SyscallId::SYS_SIGACTION => sys_sigaction(
            args[0] as isize,
            args[1] as *const SigAction,
            args[2] as *mut SigAction,
        ),

        SyscallId::SYS_SIGPROCMASK => sys_sigprocmask(
            args[0] as usize,
            args[1] as *const usize,
            args[2] as *mut usize,
            args[3],
        ),
        SyscallId::SYS_SIGRETURN => sys_sigreturn(),
        SyscallId::SYS_FUTEX => sys_futex(
            args[0] as *const u32,
            args[1] as usize,
            args[2] as u32,
            args[3] as *const u32,
            args[4] as *const u32,
            args[5] as u32,
        ),
        SyscallId::SYS_TKILL => sys_tkill(args[0], args[1]),
        SyscallId::SYS_SOCKET => Ok(1),
        SyscallId::SYS_BIND => Ok(0),
        SyscallId::SYS_LISTEN => Ok(0),
        SyscallId::SYS_ACCEPT => Ok(0),
        SyscallId::SYS_CONNECT => Ok(0),
        SyscallId::SYS_GETSOCKNAME => Ok(0),
        SyscallId::SYS_SENDTO => Ok(1),
        SyscallId::SYS_RECVFROM => sys_recvfrom(
            args[0],
            args[1] as *mut u8,
            args[2],
            args[3],
            args[4],
            args[5],
        ),
        SyscallId::SYS_SETSOCKOPT => Ok(0),
        SyscallId::SYS_MADVISE => Ok(0),

        SyscallId::SYS_SCHED_SETAFFINITY => {
            sys_sched_setaffinity(args[0] as usize, args[1] as usize, args[2] as *const u8)
        }
        SyscallId::SYS_CLOCK_NANOSLEEP => sys_clock_nanosleep(
            args[0] as usize,
            args[1] as isize,
            args[2] as *const TimeSpec,
            args[3] as *mut TimeSpec,
        ),

        SyscallId::SYS_TIMER_SETTIME => sys_timer_settime(
            args[0] as usize,
            args[1] as isize,
            args[2] as *const itimerval,
            args[3] as *mut itimerval,
        ),
        SyscallId::SYS_TIMER_GETOVERRUN => Ok(0),
        SyscallId::SYS_COPY_FILE_RANGE => sys_copy_file_range(
            args[0] as i32,
            args[1] as *mut u64,
            args[2] as i32,
            args[3] as *mut u64,
            args[4] as usize,
            args[5] as u32,
        ),
    };

    match ret {
        Ok(data) => data,
        Err(err) => {
            let errno = err as isize;
            if errno > 0 {
                -errno
            } else {
                errno
            }
        }
    }
}
