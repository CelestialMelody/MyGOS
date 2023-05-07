// #![allow(unused)]
//!
//! 系统调用号
//!

// Required.
pub const SYS_GETCWD: usize = 17;
pub const SYS_PIPE2: usize = 59;
pub const SYS_DUP: usize = 23;
pub const SYS_DUP3: usize = 24;
pub const SYS_CHDIR: usize = 49;
pub const SYS_OPENAT: usize = 56;
pub const SYS_CLOSE: usize = 57;
pub const SYS_GETDENTS64: usize = 61;
pub const SYS_READ: usize = 63;
pub const SYS_WRITE: usize = 64;
pub const SYS_LINKAT: usize = 37;
pub const SYS_UNLINKAT: usize = 35;
pub const SYS_MKDIRAT: usize = 34;
pub const SYS_UMOUNT2: usize = 39;
pub const SYS_MOUNT: usize = 40;
pub const SYS_FSTAT: usize = 80;
pub const SYS_CLONE: usize = 220;
pub const SYS_EXECVE: usize = 221;
pub const SYS_WAIT4: usize = 260;
pub const SYS_EXIT: usize = 93;
pub const SYS_GETPPID: usize = 173;
pub const SYS_GETPID: usize = 172;
pub const SYS_BRK: usize = 214;
pub const SYS_MUNMAP: usize = 215;
pub const SYS_MMAP: usize = 222;
pub const SYS_TIMES: usize = 153;
pub const SYS_UNAME: usize = 160;
pub const SYS_SCHED_YIELD: usize = 124;
pub const SYS_GETTIMEOFDAY: usize = 169;
pub const SYS_NANOSLEEP: usize = 101;

// 已实现但重复了
pub const SYS_FORK: usize = 220;
pub const SYS_EXEC: usize = 221;