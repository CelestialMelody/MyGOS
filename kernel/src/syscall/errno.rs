//! Linux 错误码，系统调用的错误都存储于 [`errno`] 中
//!
//! [`errno`]: <https://man7.org/linux/man-pages/man3/errno.3.html>

#![allow(unused)]

use thiserror::Error;

pub type Result<T> = core::result::Result<T, Errno>;

/// Error number 错误号，由内核态系统调用直接返回作为调用失败时的结果
#[derive(Debug, Error)]
pub enum Errno {
    /// Error cannot be clairified due to current implementation
    #[error("Error cannot be clairified due to current implementation")]
    UNCLEAR = -1,

    /// Operation not permitted
    #[error("Operation not permitted")]
    EPERM = 1,

    /// No such file or directory
    #[error("No such file or directory")]
    ENOENT = 2,

    /// No such process
    #[error("No such process")]
    ESRCH = 3,

    /// Interrupted system call
    #[error("Interrupted system call")]
    EINTR = 4,

    /// I/O error
    #[error("I/O error")]
    EIO = 5,

    /// No such device or address
    #[error("No such device or address")]
    ENXIO = 6,

    /// Argument list too long
    #[error("Argument list too long")]
    E2BIG = 7,

    /// Exec format error
    #[error("Exec format error")]
    ENOEXEC = 8,

    /// Bad file number
    #[error("Bad file number")]
    EBADF = 9,

    /// No child processes
    #[error("No child processes")]
    ECHILD = 10,

    /// Try again
    #[error("Try again")]
    EAGAIN = 11,

    /// Out of memory
    #[error("Out of memory")]
    ENOMEM = 12,

    /// Permission denied
    #[error("Permission denied")]
    EACCES = 13,

    /// Bad address
    #[error("Bad address")]
    EFAULT = 14,

    /// Block device required
    #[error("Block device required")]
    ENOTBLK = 15,

    /// Device or resource busy
    #[error("Device or resource busy")]
    EBUSY = 16,

    /// File exists
    #[error("File exists")]
    EEXIST = 17,

    /// Cross-device link
    #[error("Cross-device link")]
    EXDEV = 18,

    /// No such device
    #[error("No such device")]
    ENODEV = 19,

    /// Not a directory
    #[error("Not a directory")]
    ENOTDIR = 20,

    /// Is a directory
    #[error("Is a directory")]
    EISDIR = 21,

    /// Invalid argument
    #[error("Invalid argument")]
    EINVAL = 22,

    /// File table overflow
    #[error("File table overflow")]
    ENFILE = 23,

    /// Too many open files
    #[error("Too many open files")]
    EMFILE = 24,

    /// Not a typewriter
    #[error("Not a typewriter")]
    ENOTTY = 25,

    /// Text file busy
    #[error("Text file busy")]
    ETXTBSY = 26,

    /// File too large
    #[error("File too large")]
    EFBIG = 27,

    /// No space left on device
    #[error("No space left on device")]
    ENOSPC = 28,

    /// Illegal seek
    #[error("Illegal seek")]
    ESPIPE = 29,

    /// Read-only file system
    #[error("Read-only file system")]
    EROFS = 30,

    /// Too many links
    #[error("Too many links")]
    EMLINK = 31,

    /// Broken pipe
    #[error("Broken pipe")]
    EPIPE = 32,

    /// Math argument out of domain of func
    #[error("Math argument out of domain of func")]
    EDOM = 33,

    /// Math result not representable
    #[error("Math result not representable")]
    ERANGE = 34,
}
