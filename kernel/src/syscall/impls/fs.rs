//! About syscall detail: https://man7.org/linux/man-pages/dir_section_2.html

use super::super::errno::*;
use crate::fs::{chdir, make_pipe, open, File, Stdin, MNT_TABLE};
use crate::mm::{
    translated_bytes_buffer, translated_mut, translated_ref, translated_str, UserBuffer, VirtAddr,
};
use crate::return_errno;
use crate::task::{current_task, current_user_token};
use crate::task::{suspend_current_and_run_next, TaskControlBlock};
use crate::timer::{get_time, get_timeval};

use alloc::{sync::Arc, vec::Vec};
use core::mem::size_of;
// use fat32::sync_all;
use crate::fat32::sync_all;
use nix::time::{TimeSpec, TimeVal};
use nix::{
    CreateMode, Dirent, FcntlFlags, InodeTime, Kstat, OpenFlags, SeekFlags, Statfs, AT_FDCWD,
    RTC_RD_TIME, TCGETS, TCSETS, TIOCGPGRP, TIOCGWINSZ, TIOCSPGRP, UTIME_NOW, UTIME_OMIT,
};
use nix::{FdSet, Iovec};

#[cfg(feature = "time-tracer")]
use time_tracer::{time_trace, TimeTracer};

/// getcwd 17
pub fn sys_getcwd(buf: *mut u8, size: usize) -> Result {
    // 不要使用  `.is_null`, 可能会由于运行时的 const 评估造成错误的结果?
    if buf as usize == 0 {
        return_errno!(Errno::EFAULT, "buf is NULL");
    }
    if buf as usize != 0 && size == 0 {
        return_errno!(Errno::EINVAL, "buf is not NULL but size is zero");
    }
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_mut();

    let buf_vec = translated_bytes_buffer(token, buf, size);
    let mut userbuf = UserBuffer::wrap(buf_vec);
    let cwd = inner.cwd.to_string();
    let cwd_str = cwd.as_bytes();
    userbuf.write(cwd_str);
    userbuf.write_at(cwd_str.len(), &[0]); // 添加字符串末尾的\0
    Ok(buf as isize)
}

// pipe2 59
pub fn sys_pipe2(pipe: *mut i32, _flag: i32) -> Result {
    let fd0 = pipe;
    let fd1 = unsafe { pipe.add(1) };

    let task = current_task().unwrap();
    let token = current_user_token();

    let (pipe_read, pipe_write) = make_pipe();

    // fd_table mut borrow
    let mut fd_table = task.fd_table.write();
    let fd_limit = task.inner_ref().rlimit_nofile.rlim_cur;
    let read_fd = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
    if read_fd >= fd_limit {
        return_errno!(Errno::EMFILE);
    }
    fd_table[read_fd] = Some(pipe_read);

    let write_fd = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
    if write_fd >= fd_limit {
        return_errno!(Errno::EMFILE);
    }
    fd_table[write_fd] = Some(pipe_write);

    drop(fd_table);

    let fd0_phys_addr = translated_mut(token, fd0);
    let fd1_phys_addr = translated_mut(token, fd1);
    *fd0_phys_addr = read_fd as i32;
    *fd1_phys_addr = write_fd as i32;

    Ok(0)
}

// dup 23
pub fn sys_dup(old_fd: usize) -> Result {
    let task = current_task().unwrap();
    // fd_table mut borrow
    let mut fd_table = task.fd_table.write();
    let fd_limit = task.inner_ref().rlimit_nofile.rlim_cur;
    // 超出范围
    if old_fd >= fd_table.len() {
        return_errno!(Errno::EBADF, "oldfd is out of range, oldfd: {}", old_fd);
    }
    // oldfd 不存在
    if fd_table[old_fd].is_none() {
        return_errno!(Errno::EBADF, "oldfd is not exist, oldfd {}", old_fd);
    }

    let new_fd = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
    if new_fd >= fd_limit {
        return_errno!(Errno::EMFILE, "too many fd, newfd: {}", new_fd);
    }
    fd_table[new_fd] = Some(Arc::clone(fd_table[old_fd].as_ref().unwrap()));
    // println!("fd:{:?},limit:{:?}",new_fd,fd_limit);

    Ok(new_fd as isize)
}

// dup3 24
pub fn sys_dup3(old_fd: usize, new_fd: usize) -> Result {
    let task = current_task().unwrap();
    let mut fd_table = task.fd_table.write();

    // 超出范围或 oldfd 不存在
    if old_fd >= fd_table.len() || fd_table[old_fd].is_none() {
        return_errno!(Errno::EBADF);
    }

    if new_fd >= fd_table.len() {
        for _ in fd_table.len()..(new_fd + 1) {
            fd_table.push(None);
        }
    }

    fd_table[new_fd] = Some(fd_table[old_fd].as_ref().unwrap().clone());
    Ok(new_fd as isize)
}

// chdir 49
pub fn sys_chdir(path: *const u8) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let mut inner = task.inner_mut();
    let path = translated_str(token, path);
    let current_path = inner.cwd.clone();
    let new_path = current_path.cd(path.clone());
    if chdir(new_path.clone()) {
        inner.cwd = new_path.clone();
        Ok(0)
    } else {
        return_errno!(Errno::ENOENT);
    }
}

// openat 56
pub fn sys_openat(fd: i32, filename: *const u8, flags: u32, mode: u32) -> Result {
    #[cfg(feature = "time-tracer")]
    time_trace!("sys_openat");
    let task = current_task().unwrap();
    let token = current_user_token();
    let inner = task.inner_mut();
    let mut fd_table = task.fd_table.write();

    let path = translated_str(token, filename);
    let mode = CreateMode::from_bits(mode).unwrap_or(CreateMode::empty());
    let flags = OpenFlags::from_bits(flags).unwrap_or(OpenFlags::empty());
    let fd_limit = inner.rlimit_nofile.rlim_cur;
    if fd as isize == AT_FDCWD {
        let open_path = inner.get_work_path().cd(path);
        let inode = open(open_path.clone(), flags, mode)?;
        let fd = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
        if fd >= fd_limit {
            return_errno!(Errno::EMFILE);
        }
        fd_table[fd] = Some(inode);
        Ok(fd as isize)
        // } else {
        //     return_errno!(Errno::ENOENT, "try open path {:?}", open_path);
        // }
    } else {
        let dirfd = fd as usize;
        // dirfd 不合法
        if dirfd >= fd_table.len() {
            return_errno!(Errno::EINVAL);
        }
        if dirfd >= fd_limit {
            return_errno!(Errno::EMFILE);
        }
        if let Some(file) = &fd_table[dirfd] {
            let open_path = file.path().cd(path.clone());
            // target file 存在
            let tar_file = open(open_path.clone(), flags, mode)?;
            let fd = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
            if fd >= fd_limit {
                return_errno!(Errno::EMFILE);
            }
            fd_table[fd] = Some(tar_file);
            Ok(fd as isize)
            // } else {
            //     return_errno!(Errno::ENOENT, "try to open {:?}", path);
            // }
        } else {
            // dirfd 对应条目为 None
            return_errno!(Errno::ENOENT, "no such a file, fd: {}", dirfd);
        }
    }
}

// sys_close 57
pub fn sys_close(fd: usize) -> Result {
    let task = current_task().unwrap();
    let mut fd_table = task.fd_table.write();
    if fd >= fd_table.len() {
        return_errno!(Errno::EBADF, "try to close fd out of range {}", fd);
    }
    if fd_table[fd].is_none() {
        return_errno!(Errno::EBADF, "try to close fd that is not exists {}", fd);
    }
    // 把 fd 对应的值取走, 变为 None
    fd_table[fd].take();
    Ok(0)
}

// getdents64 61
pub fn sys_getdents64(fd: isize, buf: *mut u8, len: usize) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_mut();
    let work_path = inner.cwd.clone();
    let buf_vec = translated_bytes_buffer(token, buf, len);
    let mut userbuf = UserBuffer::wrap(buf_vec);
    let mut dirent = Dirent::new();
    let dent_len = size_of::<Dirent>();
    let mut total_len: usize = 0;
    let fd_table = task.fd_table.read();

    if fd == AT_FDCWD {
        let file = open(work_path.clone(), OpenFlags::O_RDONLY, CreateMode::empty())?;
        loop {
            if total_len + dent_len > len {
                break;
            }
            if file.dirent(&mut dirent) > 0 {
                userbuf.write_at(total_len, dirent.as_bytes());
                total_len += dent_len;
            } else {
                break;
            }
        }
        Ok(total_len as isize)
        // } else {
        //     return_errno!(Errno::EBADF, "could not open {:?}", work_path);
        // }
    } else {
        let fd = fd as usize;
        if let Some(file) = &fd_table[fd] {
            loop {
                if total_len + dent_len > len {
                    break;
                }
                if file.dirent(&mut dirent) > 0 {
                    userbuf.write_at(total_len, dirent.as_bytes());
                    total_len += dent_len;
                } else {
                    break;
                }
            }
            Ok(total_len as isize)
        } else {
            return_errno!(Errno::EBADF, "could not find fd {}", fd);
        }
    }
}

// read 63
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> Result {
    #[cfg(feature = "time-tracer")]
    time_trace!("sys_read");
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();

    // 文件描述符不合法
    if fd >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd is out of range, fd: {}, fd_table.len(): {}",
            fd,
            fd_table.len()
        );
    }
    if let Some(file) = &fd_table[fd] {
        // 文件不可读
        #[cfg(feature = "time-tracer")]
        time_trace!("sys_read_2");
        if !file.readable() {
            return_errno!(Errno::EINVAL, "fd is not readable, fd: {}", fd);
        }
        let file = file.clone();

        drop(fd_table); // 释放以避免死锁
        drop(task); // 需要及时释放减少引用数

        // 对 /dev/zero 的处理, 暂时先加在这里
        if file.name() == "zero" || file.name() == "ZERO" {
            let mut userbuffer = UserBuffer::wrap(translated_bytes_buffer(token, buf, len));
            let zero: Vec<u8> = (0..userbuffer.buffers.len()).map(|_| 0).collect();
            userbuffer.write(zero.as_slice());
            return Ok(userbuffer.buffers.len() as isize);
        }

        #[cfg(feature = "time-tracer")]
        time_trace!("sys_read_3");
        let file_size = file.file_size();
        let file_offset = file.offset();
        if file_size == 0 {
            warn!("sys_read: {} file_size is zero!", file.name());
        }
        let len = len.min(file_size - file_offset);
        let readsize =
            file.read_to_ubuf(UserBuffer::wrap(translated_bytes_buffer(token, buf, len))) as isize;
        Ok(readsize as isize)
    } else {
        return_errno!(Errno::EBADF, "fd is not exist, fd: {}", fd);
    }
}

// pread64 67
pub fn sys_pread64(fd: usize, buf: *const u8, len: usize, offset: usize) -> Result {
    #[cfg(feature = "time-tracer")]
    time_trace!("sys_pread");
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.write();

    // 文件描述符不合法
    if fd >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd {} is out of length of fd_table: {}",
            fd,
            fd_table.len()
        );
    }
    if let Some(file) = &fd_table[fd] {
        // 文件不可读
        if !file.readable() {
            return_errno!(Errno::EBADF, "file can't be read, fd: {}", fd);
        }
        let file = file.clone();

        drop(fd_table); // 释放以避免死锁
        drop(task); // 需要及时释放减少引用数

        // 对 /dev/zero 的处理, 暂时先加在这里
        if file.name() == "zero" || file.name() == "ZERO" {
            let mut userbuffer = UserBuffer::wrap(translated_bytes_buffer(token, buf, len));
            let zero: Vec<u8> = (0..userbuffer.buffers.len()).map(|_| 0).collect();
            userbuffer.write(zero.as_slice());
            return Ok(userbuffer.buffers.len() as isize);
        }

        let file_size = file.file_size();
        if file_size == 0 {
            warn!("sys_read: file_size is zero!");
        }
        let len = len.min(file_size - offset);
        let readsize = file.pread(
            UserBuffer::wrap(translated_bytes_buffer(token, buf, len)),
            offset,
        ) as isize;
        Ok(readsize)
    } else {
        return_errno!(Errno::EBADF, "couldn't find fd: {}", fd);
    }
}

// write 64
pub fn sys_write(fd: i32, buf: *const u8, len: usize) -> Result {
    #[cfg(feature = "time-tracer")]
    time_trace!("sys_write");
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    let memory_set = task.memory_set.read();

    // 文件描述符不合法
    if fd as usize >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd {} is out of length of fd_table: {}",
            fd,
            fd_table.len()
        );
    }

    let is_va_range_valid = memory_set.check_va_range(VirtAddr::from(buf as usize), len);
    if !is_va_range_valid {
        return_errno!(
            Errno::EFAULT,
            "buf is out of accessible address space, buf: {}",
            buf as usize
        );
    }

    if let Some(file) = &fd_table[fd as usize] {
        // 文件不可写
        if !file.writable() {
            return_errno!(
                Errno::EINVAL,
                "fd is not writable, fd: {}, filename: {}",
                fd,
                file.name()
            );
        }
        let file = file.clone();
        drop(fd_table);
        drop(memory_set);

        let write_size = file
            .write_from_ubuf(UserBuffer::wrap(translated_bytes_buffer(token, buf, len)))
            as isize;
        Ok(write_size)
    } else {
        return_errno!(Errno::EBADF, "fd is not found, fd: {}", fd);
    }
}

// pwirte64 68
pub fn sys_pwrite64(fd: i32, buf: *const u8, len: usize, offset: usize) -> Result {
    #[cfg(feature = "time-tracer")]
    time_trace!("sys_pwrite");
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    let memory_set = task.memory_set.read();

    // 文件描述符不合法
    if fd as usize >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd: {}, fd_table.len: {}, fd is out of range",
            fd,
            fd_table.len()
        );
    }

    let is_va_range_valid = memory_set.check_va_range(VirtAddr::from(buf as usize), len);
    if !is_va_range_valid {
        return_errno!(
            Errno::EFAULT,
            "buf is out of accessible address space, buf: {}",
            buf as usize
        );
    }

    if let Some(file) = &fd_table[fd as usize] {
        // 文件不可写
        if !file.writable() {
            return_errno!(
                Errno::EINVAL,
                "fd is not writable, fd: {}, filename: {}",
                fd,
                file.name()
            );
        }
        let file = file.clone();
        drop(fd_table);
        drop(memory_set);

        let write_size = file.pwrite(
            UserBuffer::wrap(translated_bytes_buffer(token, buf, len)),
            offset,
        ) as isize;
        Ok(write_size)
    } else {
        return_errno!(Errno::EBADF, "could not find fd: {}", fd);
    }
}

// linkat 37
pub fn sys_linkat(
    _old_dirfd: isize,
    _old_path: *const u8,
    _new_dirfd: isize,
    _new_path: *const u8,
    _flags: u32,
) -> Result {
    todo!()
}

// unlinkat 35
pub fn sys_unlinkat(fd: isize, path: *const u8, flags: u32) -> Result {
    let task = current_task().unwrap();
    let token = current_user_token();
    let inner = task.inner_mut();

    _ = flags;

    let path = translated_str(token, path);
    let open_path = inner.get_work_path().cd(path);

    if fd == AT_FDCWD {
        let file = open(open_path.clone(), OpenFlags::O_RDWR, CreateMode::empty())?;
        file.delete();
        Ok(0)
        // } else {
        //     return_errno!(Errno::ENOENT, "could not open: {:?}", open_path);
        // }
    } else {
        unimplemented!("in sys_unlinkat");
    }
}

// mkdirat 34
pub fn sys_mkdirat(dirfd: i32, path: *const u8, _mode: u32) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_mut();
    let fd_table = task.fd_table.read();
    let path = translated_str(token, path);
    let fd_limit = inner.rlimit_nofile.rlim_cur;
    if dirfd as isize == AT_FDCWD {
        let open_path = inner.get_work_path().cd(path);
        let _ = open(
            open_path.clone(),
            OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
            CreateMode::empty(),
        );
        // ) {
        //     Ok(0)
        // } else {
        //     return_errno!(Errno::ENOENT, "could not open: {:?}", open_path);
        // }
        Ok(0)
    } else {
        let dirfd = dirfd as usize;
        if dirfd >= fd_table.len() && dirfd >= fd_limit {
            return_errno!(Errno::EBADF, "fd {} is out of range or reach limit", dirfd);
        }
        if let Some(file) = &fd_table[dirfd] {
            let open_path = file.path().cd(path);

            let _ = open(
                open_path.clone(),
                OpenFlags::O_DIRECTROY | OpenFlags::O_CREATE,
                CreateMode::empty(),
            );
            Ok(0)
        // else {
        //     return_errno!(Errno::ENOENT, "could not open: {:?}", open_path);
        // }
        } else {
            return_errno!(Errno::EBADF, "could not find fd: {}", dirfd);
        }
    }
}

// umount2 39
pub fn sys_umount2(p_special: *const u8, flags: usize) -> Result {
    let token = current_user_token();
    let special = translated_str(token, p_special);

    match MNT_TABLE.lock().umount(special, flags as u32) {
        0 => Ok(0),
        -1 => return_errno!(Errno::EINVAL),

        _ => unreachable!(),
    }
}

// mount 40
pub fn sys_mount(
    special: *const u8,
    dir: *const u8,
    fstype: *const u8,
    flags: usize,
    data: *const u8,
) -> Result {
    let token = current_user_token();
    let special = translated_str(token, special);
    let dir = translated_str(token, dir);
    let fstype = translated_str(token, fstype);

    _ = data;

    match MNT_TABLE.lock().mount(special, dir, fstype, flags as u32) {
        0 => Ok(0),
        -1 => return_errno!(Errno::EMFILE, "mount too many"),
        _ => unreachable!(),
    }
}

// fstat 80
pub fn sys_fstat(fd: i32, buf: *mut u8) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let buf_vec = translated_bytes_buffer(token, buf, size_of::<Kstat>());
    let fd_table = task.fd_table.read();

    let mut userbuf = UserBuffer::wrap(buf_vec);
    let mut kstat = Kstat::new();
    let fd_limit = task.inner_ref().rlimit_nofile.rlim_cur;
    let dirfd = fd as usize;
    if dirfd >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd {} is out of range: {}",
            dirfd,
            fd_table.len()
        );
    }
    if dirfd >= fd_limit {
        return_errno!(Errno::EBADF, "fd {} reached limit", dirfd);
    }
    if let Some(file) = &fd_table[dirfd] {
        file.fstat(&mut kstat);
        userbuf.write(kstat.as_bytes());
        Ok(0)
    } else {
        return_errno!(Errno::EBADF, "could not find fd: {}", dirfd);
    }
}

// readv 65
pub fn sys_readv(fd: usize, iovp: *const usize, iovcnt: usize) -> Result {
    #[cfg(feature = "time-tracer")]
    time_trace!("sys_readv");
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    if fd >= fd_table.len() {
        return_errno!(Errno::EBADF, "fd {} out of range: {}", fd, fd_table.len());
    }
    if let Some(file) = &fd_table[fd] {
        if !file.readable() {
            return_errno!(
                Errno::EINVAL,
                "fd is not readable, fd: {}, filename: {}",
                fd,
                file.name()
            );
        }
        let mut addr = iovp as *const _ as usize;
        let file = file.clone();
        drop(fd_table);

        let file_size = file.file_size();
        let file_offset = file.offset();
        if file_size == 0 {
            warn!("sys_readv: file_size is zero!");
        }

        let mut total_read_len = 0;
        for _ in 0..iovcnt {
            let iov = translated_ref(token, addr as *const Iovec);

            let len = iov.iov_len.min(file_size - file_offset - total_read_len);
            // println!("[DEBUG] sys_readv iov_addr:{:x?} len:{:?},buffer_len:{:?}",iov.iov_base,iov.iov_len,len);
            total_read_len += file.read_to_ubuf(UserBuffer::wrap(translated_bytes_buffer(
                token,
                iov.iov_base as *const u8,
                len,
            )));
            addr += size_of::<Iovec>();
        }
        Ok(total_read_len as isize)
    } else {
        return_errno!(Errno::EBADF, "fd {} is not found", fd);
    }
}

// wirtev 66
pub fn sys_writev(fd: usize, iovp: *const usize, iovcnt: usize) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    // 文件描述符不合法
    if fd >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd {} is out of fd_table.len: {}",
            fd,
            fd_table.len()
        );
    }
    if let Some(file) = &fd_table[fd] {
        // 文件不可写
        if !file.writable() {
            return_errno!(
                Errno::EINVAL,
                "fd is not writable, fd: {}, filename: {}",
                fd,
                file.name()
            );
        }
        let mut addr = iovp as *const _ as usize;
        let mut total_write_len = 0;
        for _ in 0..iovcnt {
            let iov = translated_ref(token, addr as *const Iovec);
            if iov.iov_len <= 0 {
                addr += size_of::<Iovec>();
                continue;
            }
            total_write_len += file.write_from_ubuf(UserBuffer::wrap(translated_bytes_buffer(
                token,
                iov.iov_base as *const u8,
                iov.iov_len,
            )));

            addr += size_of::<Iovec>();
        }

        Ok(total_write_len as isize)
    } else {
        return_errno!(Errno::EBADF, "fd {} did not exist", fd);
    }
}

// ioctl 29
pub fn sys_ioctl(fd: i32, request: usize, argp: *mut u8) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    // 文件描述符不合法
    if fd as usize >= fd_table.len() {
        return_errno!(
            Errno::EBADF,
            "fd {} is out of fd_table.len: {}",
            fd,
            fd_table.len()
        );
    }
    match request {
        TCGETS => {}
        TCSETS => {}
        TIOCGPGRP => *translated_mut(token, argp) = 0 as u8,
        TIOCSPGRP => {}
        TIOCGWINSZ => *translated_mut(token, argp) = 0 as u8,
        RTC_RD_TIME => {}
        _ => return_errno!(Errno::EINVAL, "request {} is not supported", request),
    }
    Ok(0)
}

// fcmtl 25
pub fn sys_fcntl(fd: i32, cmd: usize, arg: Option<usize>) -> Result {
    let task = current_task().unwrap();
    let cmd = FcntlFlags::from_bits(cmd).unwrap();
    let mut fd_table = task.fd_table.write();
    match cmd {
        FcntlFlags::F_SETFL => {
            if let Some(file) = &fd_table[fd as usize] {
                file.set_flags(OpenFlags::from_bits(arg.unwrap() as u32).unwrap());
            } else {
                return_errno!(Errno::EBADF, "fd {} is found", fd);
            }
        }
        // Currently, only one such flag is defined: FD_CLOEXEC (value: 1)
        FcntlFlags::F_GETFD => {
            // Return (as the function result) the file descriptor flags; arg is ignored.
            if let Some(file) = &fd_table[fd as usize] {
                return Ok(file.available() as isize);
            } else {
                return_errno!(Errno::EBADF, "fd {} is found", fd);
            }
        }
        FcntlFlags::F_SETFD => {
            // Set the file descriptor flags to the value specified by arg.
            if let Some(file) = &fd_table[fd as usize] {
                if arg.unwrap() != 0 {
                    file.set_cloexec();
                }
            } else {
                return_errno!(Errno::EBADF, "fd {} is found", fd);
            }
        }
        FcntlFlags::F_GETFL => {
            // Return (as the function result) the file access mode and the file status flags; arg is ignored.
            // todo
            return Ok(04000);
        }
        FcntlFlags::F_DUPFD_CLOEXEC => {
            let inner = task.inner_mut();
            let start_num = arg.unwrap();
            let mut new_fd = 0;
            _ = new_fd;
            let mut tmp_fd = Vec::new();
            let fd_limit = inner.rlimit_nofile.rlim_cur;
            loop {
                new_fd = TaskControlBlock::alloc_fd(&mut fd_table, fd_limit);
                fd_table[new_fd] = Some(Arc::new(Stdin));
                if new_fd >= start_num {
                    break;
                } else {
                    tmp_fd.push(new_fd);
                }
            }
            for i in tmp_fd {
                fd_table[i].take();
            }
            fd_table[new_fd] = match &fd_table[fd as usize] {
                Some(fd) => Some(Arc::clone(fd)),
                None => return_errno!(Errno::EBADF, "fd {} is not exist", fd),
            };
            fd_table[new_fd].as_ref().unwrap().set_cloexec();
            drop(fd_table);
            return Ok(new_fd as isize);
        }
        _ => return_errno!(Errno::EINVAL, "cmd {:?} is not supported", cmd),
    }
    Ok(0)
}

// ftruncate 79
pub fn sys_newfstatat(
    dirfd: isize,
    pathname: *const u8,
    satabuf: *const usize,
    _flags: usize,
) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_mut();
    let fd_table = task.fd_table.read();
    let path = translated_str(token, pathname);

    let buf_vec = translated_bytes_buffer(token, satabuf as *const u8, size_of::<Kstat>());
    let mut userbuf = UserBuffer::wrap(buf_vec);
    let mut kstat = Kstat::new();
    let fd_limit = inner.rlimit_nofile.rlim_cur;
    // 相对路径, 在当前工作目录
    if dirfd == AT_FDCWD {
        let open_path = inner.get_work_path().cd(path);
        let inode = open(open_path.clone(), OpenFlags::O_RDONLY, CreateMode::empty())?;
        inode.fstat(&mut kstat);
        userbuf.write(kstat.as_bytes());
        Ok(0)
        // } else {
        //     return_errno!(Errno::ENOENT, "could not open {:?}", open_path);
        // }
    } else {
        let dirfd = dirfd as usize;
        if dirfd >= fd_table.len() {
            return_errno!(Errno::EBADF, "fd {} is out of fd_table_len", dirfd);
        }
        if dirfd >= fd_limit {
            return_errno!(Errno::EBADF, "too many fd, fd: {}", dirfd);
        }

        if let Some(_file) = &fd_table[dirfd] {
            let open_path = inner.get_work_path().cd(path);
            let inode = open(open_path, OpenFlags::O_RDONLY, CreateMode::empty())?;
            inode.fstat(&mut kstat);
            userbuf.write(kstat.as_bytes());
            Ok(0)
        } else {
            return_errno!(Errno::EBADF, "fd {} could not be found", dirfd);
        }
    }
}

// sendfile 71
pub fn sys_sendfile(out_fd: i32, in_fd: i32, offset: usize, _count: usize) -> Result {
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    let mut total_write_size = 0usize;
    if offset as usize != 0 {
        unimplemented!();
    } else {
        let in_file = fd_table[in_fd as usize].as_ref().unwrap();
        let out_file = fd_table[out_fd as usize].as_ref().unwrap();
        let mut data_buffer;
        loop {
            data_buffer = in_file.read_to_kspace();
            let len = data_buffer.len();
            if len == 0 {
                break;
            } else {
                out_file.write_from_kspace(&data_buffer);
                total_write_size += len;
            }
        }
        Ok(total_write_size as isize)
    }
}

// utimensat 88
pub fn sys_utimensat(
    dirfd: isize,
    pathname: *const u8,
    times: *const [TimeSpec; 2],
    flags: usize,
) -> Result {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_mut();
    let fd_table = task.fd_table.read();

    let mut time0 = TimeSpec::empty();
    let mut time1 = TimeSpec::empty();

    let time = TimeSpec::from_ticks(get_time());
    let mut time_info = InodeTime::empty();

    if times as usize != 0 {
        let times = translated_ref(token, times);
        time0 = times[0];
        time1 = times[1];
        // { info!("utimensat: {:?}, {:?}, {:x?}, {:x?}",dirfd, pathname, time0, time1); }
        match time0.tv_nsec {
            UTIME_NOW => {
                time_info.access_time = time.tv_sec;
            }
            UTIME_OMIT => {
                time_info.access_time = 0;
            }
            _ => {
                time_info.access_time = time0.tv_sec;
            }
        }
        match time1.tv_nsec {
            UTIME_NOW => {
                time_info.modify_time = time.tv_sec;
            }
            UTIME_OMIT => {
                time_info.modify_time = 0;
            }
            _ => {
                time_info.modify_time = time1.tv_sec;
            }
        }
    }

    _ = flags;

    if dirfd == AT_FDCWD {
        if pathname as usize == 0 {
            unimplemented!();
        } else {
            let pathname = translated_str(token, pathname);
            let path = inner.get_work_path().cd(pathname);
            let file = open(
                path,
                OpenFlags::O_RDWR | OpenFlags::O_CREATE,
                CreateMode::empty(),
            );
            // {
            //     Ok(0)
            // } else {
            //     return_errno!(Errno::UNCLEAR);
            //     // Ok(-ENOENT)
            // }
            Ok(0)
        }
    } else {
        if pathname as usize == 0 {
            if dirfd >= fd_table.len() as isize || dirfd < 0 {
                return Ok(0);
            }
            if let Some(file) = &fd_table[dirfd as usize] {
                file.set_time(time_info);
                Ok(0)
            } else {
                return_errno!(Errno::DISCARD);
            }
        } else {
            unimplemented!();
        }
    }
}

// Currently, only renaming of files within the current directory is supported.
// renameat2 276
pub fn sys_renameat2(
    old_dirfd: isize,
    old_path: *const u8,
    new_dirfd: isize,
    new_path: *const u8,
    _flags: u32,
) -> Result {
    let task = current_task().unwrap();
    let token = current_user_token();
    let inner = task.inner_mut();
    let old_path = translated_str(token, old_path);
    let new_path = translated_str(token, new_path);

    let old_path = inner.get_work_path().cd(old_path);

    if old_dirfd == AT_FDCWD {
        let old_file = open(old_path, OpenFlags::O_RDWR, CreateMode::empty())?;
        let flag = {
            if old_file.is_dir() {
                OpenFlags::O_RDWR | OpenFlags::O_CREATE | OpenFlags::O_DIRECTROY
            } else {
                OpenFlags::O_RDWR | OpenFlags::O_CREATE
            }
        };
        if new_dirfd == AT_FDCWD {
            let new_path = inner.get_work_path().cd(new_path);
            old_file.rename(new_path, flag);
            Ok(0)
        } else {
            unimplemented!();
        }
        // } else {
        //     panic!("can't find old file");
        // }
    } else {
        unimplemented!();
    }
}

// lseek 62
pub fn sys_lseek(fd: usize, off_t: isize, whence: usize) -> Result {
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    // 文件描述符不合法
    if fd >= fd_table.len() {
        return_errno!(Errno::EBADF, "fd {} is out of bounds", fd_table.len());
    }

    if let Some(file) = &fd_table[fd] {
        let flag = SeekFlags::from_bits(whence).unwrap();
        match flag {
            SeekFlags::SEEK_SET => {
                if off_t < 0 {
                    return_errno!(Errno::EINVAL, "offset is negtive");
                }
                file.seek(off_t as usize);
                Ok(off_t as isize)
            }
            SeekFlags::SEEK_CUR => {
                let current_offset = file.offset() as isize;
                if current_offset + off_t < 0 {
                    return_errno!(Errno::EINVAL, "new offset is negtive");
                }

                file.seek((off_t + current_offset) as usize);
                Ok((off_t + current_offset) as isize)
            }
            SeekFlags::SEEK_END => {
                let end = file.file_size() as isize;
                if end + off_t < 0 {
                    return_errno!(Errno::EINVAL, "new offset is negtive");
                }

                file.seek((end + off_t) as usize);
                Ok((end + off_t) as isize)
            }
            // flag wrong
            _ => panic!("sys_lseek: unsupported whence!"),
        }
    } else {
        // file not exists
        return_errno!(Errno::DISCARD);
        // -3
    }
}

// readlinkat 78
pub fn sys_readlinkat(dirfd: isize, pathname: *const u8, buf: *const u8, bufsiz: usize) -> Result {
    if dirfd == AT_FDCWD {
        let token = current_user_token();
        let path = translated_str(token, pathname);
        // println!("readlinkat path:{:?}", path);
        if path.as_str() != "/proc/self/exe" {
            panic!("sys_readlinkat: pathname not support");
        }
        let mut userbuf = UserBuffer::wrap(translated_bytes_buffer(token, buf, bufsiz));
        let procinfo = "/lua\0";
        userbuf.write(procinfo.as_bytes());
        let len = procinfo.len() - 1;
        return Ok(len as isize);
    } else {
        panic!("sys_readlinkat: fd not support");
    }
}

// sync 81
pub fn sys_sync() -> Result {
    sync_all();
    Ok(0)
}

// ftruncate64 46
pub fn sys_ftruncate64(fd: usize, length: usize) -> Result {
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    if let Some(file) = &fd_table[fd] {
        file.truncate(length);
        Ok(0)
    } else {
        return_errno!(Errno::DISCARD);
    }
}

// pselct6 72
pub fn sys_pselect6(
    nfds: usize,
    readfds: *mut u8,
    writefds: *mut u8,
    exceptfds: *mut u8,
    timeout: *mut usize,
) -> Result {
    let token = current_user_token();
    let mut r_ready_count = 0;
    let mut w_ready_count = 0;
    let mut e_ready_count = 0;

    let mut timer_interval = TimeVal::new();
    unsafe {
        let sec = translated_ref(token, timeout);
        let usec = translated_ref(token, timeout.add(1));
        timer_interval.sec = *sec;
        timer_interval.usec = *usec;
    }
    let timer = timer_interval + get_timeval();

    let mut rfd_set = FdSet::new();
    let mut wfd_set = FdSet::new();

    let mut ubuf_rfds = {
        if readfds as usize != 0 {
            UserBuffer::wrap(translated_bytes_buffer(token, readfds, size_of::<FdSet>()))
        } else {
            UserBuffer::empty()
        }
    };
    ubuf_rfds.read(rfd_set.as_bytes_mut());

    let mut ubuf_wfds = {
        if writefds as usize != 0 {
            UserBuffer::wrap(translated_bytes_buffer(token, writefds, size_of::<FdSet>()))
        } else {
            UserBuffer::empty()
        }
    };
    ubuf_wfds.read(wfd_set.as_bytes_mut());

    let mut ubuf_efds = {
        if exceptfds as usize != 0 {
            UserBuffer::wrap(translated_bytes_buffer(
                token,
                exceptfds,
                size_of::<FdSet>(),
            ))
        } else {
            UserBuffer::empty()
        }
    };

    let mut r_has_nready = false;
    let mut w_has_nready = false;
    let mut r_all_ready = false;
    let mut w_all_ready = false;

    let mut rfd_vec: Vec<usize> = rfd_set.get_fd_vec();
    let mut wfd_vec: Vec<usize> = wfd_set.get_fd_vec();

    loop {
        // handle read fd set
        let task = current_task().unwrap();
        let fd_table = task.fd_table.read();
        if readfds as usize != 0 && !r_all_ready {
            for i in 0..rfd_vec.len() {
                let fd = rfd_vec[i];
                if fd >= nfds || fd == 1024 {
                    continue;
                }

                if fd > fd_table.len() {
                    return_errno!(
                        Errno::EBADF,
                        "fd {} is out of fd_table.len: {}",
                        fd,
                        fd_table.len()
                    );
                }
                if fd_table[fd].is_none() {
                    return_errno!(Errno::EBADF, "fd {} could not be found", fd);
                }
                let fdescript = fd_table[fd].as_ref().unwrap();
                if fdescript.r_ready() {
                    r_ready_count += 1;
                    rfd_set.set_fd(fd);
                    // marked for being ready
                    rfd_vec[i] = 1024;
                } else {
                    rfd_set.clear_fd(fd);
                    r_has_nready = true;
                }
            }
            if !r_has_nready {
                r_all_ready = true;
                ubuf_rfds.write(rfd_set.as_bytes());
            }
        }

        // handle write fd set
        if writefds as usize != 0 && !w_all_ready {
            if wfd_vec.len() == 0 {
                wfd_vec = wfd_set.get_fd_vec();
            }

            for i in 0..wfd_vec.len() {
                let fd = wfd_vec[i];
                if fd >= nfds || fd == 1024 {
                    continue;
                }
                if fd > fd_table.len() {
                    return_errno!(
                        Errno::EBADF,
                        "fd {} is out of fd_table.len: {}",
                        fd,
                        fd_table.len()
                    );
                }
                if fd_table[fd].is_none() {
                    return_errno!(Errno::EBADF, "fd {} could not be found", fd);
                }
                let fdescript = fd_table[fd].as_ref().unwrap();
                if fdescript.w_ready() {
                    w_ready_count += 1;
                    wfd_set.set_fd(fd);
                    wfd_vec[i] = 1024;
                } else {
                    wfd_set.clear_fd(fd);
                    w_has_nready = true;
                }
            }
            if !w_has_nready {
                w_all_ready = true;
                ubuf_wfds.write(wfd_set.as_bytes());
            }
        }

        /* Cannot handle exceptfds for now */
        if exceptfds as usize != 0 {
            let mut efd_set = FdSet::new();
            ubuf_efds.read(efd_set.as_bytes_mut());
            e_ready_count = efd_set.count() as isize;
            efd_set.clear_all();
            ubuf_efds.write(efd_set.as_bytes());
        }

        // return anyway
        // return r_ready_count + w_ready_count + e_ready_count;
        // if there are some fds not ready, just wait until time up
        if r_has_nready || w_has_nready {
            r_has_nready = false;
            w_has_nready = false;
            let time_remain = get_timeval() - timer;
            if time_remain.is_zero() {
                drop(fd_table);
                drop(task);
                // suspend may nerver end, pipe read, timer
                suspend_current_and_run_next();
            } else {
                ubuf_rfds.write(rfd_set.as_bytes());
                ubuf_wfds.write(wfd_set.as_bytes());
                break;
            }
        } else {
            break;
        }
    }
    Ok((r_ready_count + w_ready_count + e_ready_count) as isize)
}

// statfs 43
pub fn sys_statfs(path: *const u8, buf: *const u8) -> Result {
    let token = current_user_token();
    let mut userbuf = UserBuffer::wrap(translated_bytes_buffer(token, buf, size_of::<Statfs>()));
    userbuf.write(Statfs::new().as_bytes());
    Ok(0)
}

pub fn sys_copy_file_range(
    fd_in: i32,
    off_in: *mut u64,
    fd_out: i32,
    off_out: *mut u64,
    len: usize,
    flags: u32,
) -> Result {
    Ok(0)
}
