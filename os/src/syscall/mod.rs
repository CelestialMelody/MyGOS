const SYSCALL_PIPE:     usize = 60;
const SYSCALL_YIELD:    usize = 124;
const SYSCALL_KILL:     usize = 129;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_FORK:     usize = 220;
const SYSCALL_EXEC:     usize = 221;
const SYSCALL_WAITPID:  usize = 260;

// new imported syscalls
pub const SYSCALL_GETCWD: usize = 17;
pub const SYSCALL_PIPE2: usize = 59;
pub const SYSCALL_DUP: usize = 23;
pub const SYSCALL_DUP3: usize = 24;
pub const SYSCALL_CHDIR: usize = 49;
pub const SYSCALL_OPENAT: usize = 56;
pub const SYSCALL_CLOSE: usize = 57;
pub const SYSCALL_GETDENTS64: usize = 61;
pub const SYSCALL_READ: usize = 63;
pub const SYSCALL_WRITE: usize = 64;
pub const SYSCALL_LINKAT: usize = 37;
pub const SYSCALL_UNLINKAT: usize = 35;
pub const SYSCALL_MKDIRAT: usize = 34;
pub const SYSCALL_UMOUNT2: usize = 39;
pub const SYSCALL_MOUNT: usize = 40;
pub const SYSCALL_FSTAT: usize = 80;
pub const SYSCALL_CLONE: usize = 220;
pub const SYSCALL_EXECVE: usize = 221;
pub const SYSCALL_WAIT4: usize = 260;
pub const SYSCALL_EXIT: usize = 93;
pub const SYSCALL_GETPPID: usize = 173;
pub const SYSCALL_GETPID: usize = 172;
pub const SYSCALL_BRK: usize = 214;
pub const SYSCALL_MUNMAP: usize = 215;
pub const SYSCALL_MMAP: usize = 222;
pub const SYSCALL_TIMES: usize = 153;
pub const SYSCALL_UNAME: usize = 160;
pub const SYSCALL_SCHED_YIELD: usize = 124;
pub const SYSCALL_GETTIMEOFDAY: usize = 169;
pub const SYSCALL_NANOSLEEP: usize = 101;

mod fs;         // 文件读写模块
mod process;    // 进程控制模块

use fs::*;
use process::*;

/// 系统调用分发函数
pub fn syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    match syscall_id {
        SYSCALL_GETCWD =>   sys_getcwd(args[0] as *mut u8, args[1] as usize),
        SYSCALL_DUP =>      sys_dup(args[0]),
        SYSCALL_DUP3 =>     sys_dup3(args[0] as usize, args[1] as usize),
        SYSCALL_MKDIRAT =>  sys_mkdirat(args[0] as isize, args[1] as *const u8, args[2] as u32),
        SYSCALL_UNLINKAT=>  sys_unlinkat(args[0] as isize, args[1] as *const u8, args[2] as u32),
        SYSCALL_UMOUNT2=>   sys_umount(args[0] as *const u8, args[1] as usize),
        SYSCALL_MOUNT=>     sys_mount(args[0] as *const u8, args[1] as *const u8, args[2] as *const u8, args[3] as usize, args[4] as *const u8),
        SYSCALL_CHDIR=>     sys_chdir(args[0] as *const u8),
        SYSCALL_OPENAT =>   sys_openat(args[0] as isize, args[1] as *const u8, args[2] as u32, args[3] as u32),
        SYSCALL_CLOSE =>    sys_close(args[0]),
        SYSCALL_PIPE =>     sys_pipe(args[0] as *mut u32,args[1]),
        SYSCALL_GETDENTS64 => sys_getdents64(args[0] as isize, args[1] as *mut u8, args[2] as usize),
        SYSCALL_READ =>     sys_read(args[0], args[1] as *const u8, args[2]),
        SYSCALL_WRITE =>    sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_FSTAT=>     sys_fstat(args[0] as isize, args[1] as *mut u8),
        SYSCALL_EXIT =>     sys_exit(args[0] as i32),
        SYSCALL_NANOSLEEP=> sys_nanosleep(args[0] as *const u8),
        SYSCALL_YIELD =>    sys_yield(),
        SYSCALL_KILL =>     sys_kill(args[0], args[1] as u32),
        SYSCALL_TIMES =>    sys_times(args[0] as *const u8),
        SYSCALL_UNAME =>    sys_uname(args[0] as *const u8),
        SYSCALL_GET_TIME => sys_get_time(args[0] as *const u8),
        SYSCALL_GETPID =>   sys_getpid(),
        SYSCALL_FORK =>     sys_fork(args[0] as usize, args[1] as  usize, args[2] as  usize, args[3] as  usize, args[4] as usize),
        SYSCALL_EXEC =>     sys_exec(args[0] as *const u8, args[1] as *const usize),
        SYSCALL_MMAP=>      sys_mmap(args[0] as usize, args[1] as usize, args[2] as usize, args[3] as usize, args[4] as isize, args[5] as usize),
        SYSCALL_MUNMAP =>   sys_munmap(args[0] as usize, args[1] as usize),
        SYSCALL_WAITPID =>  sys_waitpid(args[0] as isize, args[1] as *mut i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}

