use core::{
    cmp::{self, min},
    mem::size_of,
    task::RawWaker,
};

use alloc::{
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use bitflags::Flag;
use nix::{
    CreateMode, Dirent, InodeTime, Kstat, OpenFlags, SeekFlags, StatMode, Statfs, TimeSpec,
    NAME_LIMIT, S_IFCHR, S_IFDIR, S_IFREG, UTIME_OMIT,
};
use path::AbsolutePath;
use spin::{Mutex, RwLock};

use crate::{mm::UserBuffer, syscall::impls::Errno};

use super::{file, find_parent_dir, open, File};

pub struct RamFs {
    root: Arc<RamDirInner>,
}

impl RamFs {
    pub fn new() -> Self {
        let inner = Arc::new(RamDirInner {
            name: Mutex::new(String::from("/")),
            flags: Mutex::new(OpenFlags::O_DIRECTORY),
            rw: Mutex::new(RWablity::ReadWrite),
            children: Mutex::new(Vec::new()),
            dir_path: Mutex::new(AbsolutePath::from_str("/")),
        });
        Self { root: inner }
    }
}

impl RamFs {
    pub fn root_dir(&self, mi: MountedInfo) -> Arc<RamDir> {
        Arc::new(RamDir {
            inner: self.root.clone(),
            dents_off: Mutex::new(0),
            mi,
        })
    }

    pub fn name(&self) -> &str {
        "ramfs"
    }
}

pub struct RamDirInner {
    name: Mutex<String>,
    rw: Mutex<RWablity>,
    flags: Mutex<OpenFlags>,
    children: Mutex<Vec<FileContainer>>,
    dir_path: Mutex<AbsolutePath>,
}

impl RamDirInner {
    fn name(&self) -> String {
        self.name.lock().clone()
    }
    fn rename(&self, new_name: String) {
        *self.name.lock() = new_name;
    }
    fn set_path(&self, path: AbsolutePath) {
        *self.dir_path.lock() = path;
    }
    fn get_path(&self) -> AbsolutePath {
        self.dir_path.lock().clone()
    }
    fn rw(&self) -> RWablity {
        let flags = *self.flags.lock();
        let rw = if flags.contains(OpenFlags::O_RDONLY) {
            RWablity::ReadOnly
        } else if flags.contains(OpenFlags::O_WRONLY) {
            RWablity::WriteOnly
        } else {
            RWablity::ReadWrite
        };
        *self.rw.lock() = rw;
        // rw derive copy, so we can use rw directly.
        rw
    }
}

// TODO 设计有点问题 set flags 时更改?
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum RWablity {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

// TODO: use frame insteads of Vec.
pub struct RamFileInner {
    name: Mutex<String>,
    rw: Mutex<RWablity>,
    content: Mutex<Vec<u8>>,
    flags: Mutex<OpenFlags>,
    dir_path: Mutex<AbsolutePath>,
    // times: Mutex<[TimeSpec; 3]>, // ctime, atime, mtime.
    times: Mutex<InodeTime>,
}

impl RamFileInner {
    fn name(&self) -> String {
        self.name.lock().clone()
    }
    fn rename(&self, new_name: String) {
        *self.name.lock() = new_name;
    }
    fn set_path(&self, path: AbsolutePath) {
        *self.dir_path.lock() = path;
    }
    fn get_path(&self) -> AbsolutePath {
        self.dir_path.lock().clone()
    }
    fn rw(&self) -> RWablity {
        let flags = *self.flags.lock();
        let rw = if flags.contains(OpenFlags::O_RDONLY) {
            RWablity::ReadOnly
        } else if flags.contains(OpenFlags::O_WRONLY) {
            RWablity::WriteOnly
        } else {
            RWablity::ReadWrite
        };
        *self.rw.lock() = rw;
        rw
    }
}

pub enum FileContainer {
    File(Arc<RamFileInner>),
    Dir(Arc<RamDirInner>),
}

impl Clone for FileContainer {
    fn clone(&self) -> Self {
        match self {
            FileContainer::File(file) => FileContainer::File(file.clone()),
            FileContainer::Dir(dir) => FileContainer::Dir(dir.clone()),
        }
    }
}

impl FileContainer {
    #[inline]
    pub fn filename(&self) -> String {
        match self {
            FileContainer::File(file) => file.name(),
            FileContainer::Dir(dir) => dir.name(),
        }
    }
    pub fn to_dir(&self, mi: MountedInfo) -> Arc<RamDir> {
        match self {
            FileContainer::Dir(dir) => Arc::new(RamDir {
                inner: dir.clone(),
                dents_off: Mutex::new(0),
                mi,
            }),
            _ => panic!("not a dir"),
        }
    }
    pub fn to_file(&self, mi: MountedInfo) -> Arc<RamFile> {
        match self {
            FileContainer::File(file) => Arc::new(RamFile {
                inner: file.clone(),
                offset: Mutex::new(0),
                mi,
            }),
            _ => panic!("not a file"),
        }
    }
    #[inline]
    pub fn to_inode(&self, mi: MountedInfo) -> Arc<dyn File> {
        match self {
            FileContainer::File(file) => Arc::new(RamFile {
                inner: file.clone(),
                offset: Mutex::new(0),
                mi,
            }),
            FileContainer::Dir(dir) => Arc::new(RamDir {
                inner: dir.clone(),
                dents_off: Mutex::new(0),
                mi,
            }),
        }
    }
}

pub struct RamDir {
    inner: Arc<RamDirInner>,
    mi: MountedInfo,
    dents_off: Mutex<usize>,
}

impl File for RamDir {
    fn available(&self) -> bool {
        true
    }
    fn is_dir(&self) -> bool {
        true
    }
    // TODO
    fn dirent(&self, dirent: &mut Dirent) -> isize {
        let children = self.inner.children.lock();
        let idx = self.offset();
        if idx == children.len() {
            return 0;
        }
        let file = &children[idx];
        let filename = file.filename();
        let file_bytes = filename.as_bytes();
        dirent.d_ino = self.fid() as usize;
        dirent.d_off =
            size_of::<Dirent>() as isize - NAME_LIMIT as isize + file_bytes.len() as isize;
        dirent.d_reclen = size_of::<Dirent>() as u16;
        dirent.d_type = 0; // 0 d_type is file
        dirent.d_name[..file_bytes.len()].copy_from_slice(file_bytes);
        dirent.d_name[file_bytes.len()] = b'\0';
        self.seek(idx + 1);
        size_of::<Dirent>() as isize
    }
    fn getdents(&self, buf: &mut [u8]) -> Result<isize, Errno> {
        self.getdents(buf)
    }
    fn seek(&self, idx: usize) {
        *self.dents_off.lock() = idx;
    }
    fn name(&self) -> String {
        self.inner.name()
    }
    fn fid(&self) -> u64 {
        self.mi.fs_id as u64
    }
    fn file_size(&self) -> usize {
        unimplemented!()
    }
    fn flags(&self) -> OpenFlags {
        *self.inner.flags.lock()
    }
    fn set_flags(&self, flag: OpenFlags) {
        *self.inner.flags.lock() = flag;
    }
    fn fstat(&self, _stat: &mut Kstat) {
        self.stat(_stat).unwrap();
    }
    fn offset(&self) -> usize {
        *self.dents_off.lock()
    }
    fn path(&self) -> AbsolutePath {
        self.inner.dir_path.lock().clone()
    }
    fn set_cloexec(&self) {
        let mut flags = self.inner.flags.lock();
        *flags |= OpenFlags::O_CLOEXEC;
    }
    fn set_time(&self, time_info: InodeTime) {
        unimplemented!()
    }
    fn time(&self) -> InodeTime {
        unimplemented!()
    }
    fn kernel_read_with_offset(&self, _offset: usize, _len: usize) -> Vec<u8> {
        unimplemented!()
    }
    fn read_at_direct(&self, _offset: usize, _len: usize) -> Vec<u8> {
        unimplemented!()
    }
    fn pread(&self, _buf: UserBuffer, _offset: usize) -> usize {
        unimplemented!()
    }
    fn pwrite(&self, _buf: UserBuffer, _offset: usize) -> usize {
        unimplemented!()
    }
    fn read_to_kspace(&self) -> Vec<u8> {
        unimplemented!()
    }
    fn write_from_direct(&self, _offset: usize, _data: &Vec<u8>) -> usize {
        unimplemented!()
    }
    fn write_from_kspace(&self, _data: &Vec<u8>) -> usize {
        unimplemented!()
    }
    fn write_from_ubuf(&self, buf: UserBuffer) -> usize {
        unimplemented!()
    }
    fn w_ready(&self) -> bool {
        unimplemented!()
    }
    fn r_ready(&self) -> bool {
        unimplemented!()
    }
    fn read_to_ubuf(&self, buf: UserBuffer) -> usize {
        unimplemented!()
    }
    fn truncate(&self, _new_length: usize) {
        unimplemented!()
    }
    fn readable(&self) -> bool {
        let rw = *self.inner.rw.lock();
        rw == RWablity::ReadOnly || rw == RWablity::ReadWrite
    }
    fn writable(&self) -> bool {
        let rw = *self.inner.rw.lock();
        rw == RWablity::WriteOnly || rw == RWablity::ReadWrite
    }
    fn delete(&self) -> usize {
        let path = self.path();
        let parent = find_parent_dir(path.clone()).unwrap();
        parent.remove(self.name()).unwrap();
        0
    }
    fn rename(&self, new_path: AbsolutePath, _flag: OpenFlags) {
        let new_parent = find_parent_dir(new_path.clone()).unwrap();
        let old_parent = find_parent_dir(self.path()).unwrap();
        let inner = self.inner.clone();
        let old_name = self.name();
        let new_name = new_path.last();
        old_parent.remove(old_name).unwrap();
        inner.set_path(new_path);
        inner.rename(new_name);
        new_parent.add_from_container(FileContainer::Dir(inner));
    }
}

impl RamDir {
    pub fn open(&self, name: &str) -> Option<FileContainer> {
        self.inner
            .children
            .lock()
            .iter()
            .find(|x| x.filename() == name)
            .cloned()
    }

    pub fn open_dir(&self, name: &str) -> Option<Arc<RamDir>> {
        self.inner.children.lock().iter().find_map(|x| match x {
            FileContainer::Dir(dir) if dir.name() == name => Some(x.to_dir(self.mi.clone())),
            _ => None,
        })
    }

    fn add_from_container(&self, container: FileContainer) {
        self.inner.children.lock().push(container);
    }

    // pub fn touch(&self, name: &str, flags: OpenFlags) -> VfsResult<Arc<dyn File>> {
    pub fn touch(&self, name: &str, flags: OpenFlags) -> Result<Arc<dyn File>, Errno> {
        // Find file, return VfsError::AlreadyExists if file exists
        self.inner
            .children
            .lock()
            .iter()
            .find(|x| x.filename() == name)
            // .map_or(Ok(()), |_| Err(VfsError::AlreadyExists))?;
            .map_or(Ok(()), |_| Err(Errno::EEXIST))?;

        // TODO
        let rw = if flags.contains(OpenFlags::O_WRONLY) {
            RWablity::WriteOnly
        } else {
            RWablity::ReadWrite
        };

        let new_inner = Arc::new(RamFileInner {
            name: Mutex::new(String::from(name)),
            rw: Mutex::new(rw),
            flags: Mutex::new(flags),
            content: Mutex::new(Vec::new()),
            dir_path: {
                let path = self.inner.get_path().to_string();
                let new_path = format!("{}/{}", path, self.inner.name());
                Mutex::new(AbsolutePath::from_string(new_path))
            },
            times: Mutex::new(InodeTime::empty()),
        });

        let new_file = Arc::new(RamFile {
            inner: new_inner.clone(),
            offset: Mutex::new(0),
            mi: self.mi.clone(),
        });

        self.inner
            .children
            .lock()
            .push(FileContainer::File(new_inner));

        Ok(new_file)
    }

    // pub fn mkdir(&self, name: &str, flags: OpenFlags) -> VfsResult<Arc<dyn File>> {
    pub fn mkdir(&self, name: &str, flags: OpenFlags) -> Result<Arc<dyn File>, Errno> {
        // Find file, return VfsError::AlreadyExists if file exists
        self.inner
            .children
            .lock()
            .iter()
            .find(|x| x.filename() == name)
            // .map_or(Ok(()), |_| Err(VfsError::AlreadyExists))?;
            .map_or(Ok(()), |_| Err(Errno::EEXIST))?;

        let rw = if flags.contains(OpenFlags::O_RDONLY) {
            RWablity::ReadOnly
        } else if flags.contains(OpenFlags::O_WRONLY) {
            RWablity::WriteOnly
        } else {
            RWablity::ReadWrite
        };
        let new_inner = Arc::new(RamDirInner {
            name: Mutex::new(String::from(name)),
            flags: Mutex::new(flags),
            rw: Mutex::new(rw),
            children: Mutex::new(Vec::new()),
            dir_path: {
                let path = self.inner.get_path().to_string();
                let new_path = format!("{}/{}", path, self.inner.name());
                Mutex::new(AbsolutePath::from_string(new_path))
            },
        });

        let new_dir = Arc::new(RamDir {
            inner: new_inner.clone(),
            dents_off: Mutex::new(0),
            mi: self.mi.clone(),
        });

        self.inner
            .children
            .lock()
            .push(FileContainer::Dir(new_inner));

        Ok(new_dir)
    }

    pub fn rmdir(&self, name: &str) -> VfsResult<()> {
        // TODO: identify whether the dir is empty(through metadata.childrens)
        // return DirectoryNotEmpty if not empty.
        let len = self
            .inner
            .children
            .lock()
            .drain_filter(|x| match x {
                FileContainer::Dir(x) => x.name() == name,
                _ => false,
            })
            .count();
        match len > 0 {
            true => Ok(()),
            false => Err(VfsError::FileNotFound),
        }
    }

    pub fn read_dir(&self) -> VfsResult<Vec<DirEntry>> {
        Ok(self
            .inner
            .children
            .lock()
            .iter()
            .map(|x| match x {
                FileContainer::File(file) => DirEntry {
                    filename: file.name().to_string(),
                    len: file.content.lock().len(),
                    file_type: FileType::File,
                },
                FileContainer::Dir(dir) => DirEntry {
                    filename: dir.name().to_string(),
                    len: 0,
                    file_type: FileType::Directory,
                },
            })
            .collect())
    }

    pub fn remove(&self, name: String) -> VfsResult<()> {
        let len = self
            .inner
            .children
            .lock()
            .drain_filter(|x| match x {
                FileContainer::File(x) => x.name() == name,
                FileContainer::Dir(x) => x.name() == name,
            })
            .count();
        match len > 0 {
            true => Ok(()),
            false => Err(VfsError::FileNotFound),
        }
    }

    pub fn metadata(&self) -> VfsResult<Metadata> {
        Ok(Metadata {
            filename: self.inner.name().to_string(),
            inode: 0,
            file_type: FileType::Directory,
            size: 0,
            childrens: self.inner.children.lock().len(),
        })
    }

    pub fn stat(&self, stat: &mut Kstat) -> VfsResult<()> {
        stat.st_dev = self.mi.fs_id as u64;
        stat.st_ino = 1;
        stat.st_mode = S_IFDIR;
        stat.st_nlink = 1;
        stat.st_uid = 0;
        stat.st_gid = 0;
        stat.st_rdev = 0;
        stat.st_size = 0;
        stat.st_blksize = 512;
        stat.st_blocks = 0;
        stat.st_atime_sec = 0;
        stat.st_atime_nsec = 0;
        stat.st_mtime_sec = 0;
        stat.st_mtime_nsec = 0;
        stat.st_ctime_sec = 0;
        stat.st_ctime_nsec = 0;

        Ok(())
    }
    // 似乎可以通过两次系统调用完成offset设置（从第一个到最后一个，再从最后一个到第一个）
    pub fn getdents(&self, buffer: &mut [u8]) -> Result<isize, Errno> {
        let buf_ptr = buffer.as_mut_ptr() as usize;
        let len = buffer.len();
        let mut ptr: usize = buf_ptr;
        let mut finished = 0;
        let pre_idx = self.offset();
        for (i, x) in self.inner.children.lock().iter().enumerate().skip(pre_idx) {
            let filename = x.filename();
            let file_bytes = filename.as_bytes();
            let current_len = size_of::<Dirent>() + file_bytes.len() + 1;
            if len - (ptr - buf_ptr) < current_len {
                break;
            }

            // let dirent = c2rust_ref(ptr as *mut Dirent);
            let dirent: &mut Dirent = unsafe { (ptr as *mut Dirent).as_mut() }.unwrap();

            dirent.d_ino = 0;
            dirent.d_off = current_len as isize;
            dirent.d_reclen = current_len as u16;
            dirent.d_type = 0; // 0 d_type is file

            let buffer = unsafe {
                core::slice::from_raw_parts_mut(dirent.d_name.as_mut_ptr(), file_bytes.len() + 1)
            };
            buffer[..file_bytes.len()].copy_from_slice(file_bytes);
            buffer[file_bytes.len()] = b'\0';
            ptr = ptr + current_len;
            finished = i + 1;
        }
        self.seek(finished);
        Ok(ptr as isize - buf_ptr as isize)
    }
}

pub struct RamFile {
    inner: Arc<RamFileInner>,
    mi: MountedInfo,
    offset: Mutex<usize>,
}

impl RamFile {
    fn read(&self, buffer: &mut [u8]) -> VfsResult<usize> {
        let offset = self.offset();
        let file_size = self.inner.content.lock().len();
        match offset >= file_size {
            true => Ok(0),
            false => {
                let read_len = min(buffer.len(), file_size - offset);
                let content = self.inner.content.lock();
                buffer[..read_len].copy_from_slice(&content[offset..(offset + read_len)]);
                self.seek(offset + read_len);
                Ok(read_len)
            }
        }
    }

    fn write(&self, buffer: &[u8]) -> VfsResult<usize> {
        let offset = self.offset();
        let file_size = self.inner.content.lock().len();
        let wsize = buffer.len();

        let part1 = cmp::min(file_size - offset, wsize);
        let mut content = self.inner.content.lock();
        content[offset..offset + part1].copy_from_slice(&buffer[..part1]);
        // extend content if offset + buffer > content.len()
        content.extend_from_slice(&buffer[part1..]);

        self.seek(offset + wsize);
        Ok(wsize)
    }

    fn seek(&self, new_off: usize) {
        *self.offset.lock() = new_off;
    }

    fn truncate(&self, size: usize) -> VfsResult<()> {
        self.inner.content.lock().drain(size..);
        Ok(())
    }

    fn metadata(&self) -> VfsResult<Metadata> {
        Ok(Metadata {
            filename: self.inner.name().to_string(),
            inode: 0,
            file_type: FileType::File,
            size: self.inner.content.lock().len(),
            childrens: 0,
        })
    }

    fn stat(&self, stat: &mut Kstat) -> VfsResult<()> {
        let st_mode = if self.name() == "zero"
            || self.name() == "null"
            || self.name() == "NULL"
            || self.name() == "ZERO"
        {
            // StatMode::CHAR
            S_IFCHR
        } else {
            S_IFREG
        };
        stat.st_dev = self.mi.fs_id as u64;
        stat.st_ino = 1;
        stat.st_mode = st_mode as u32;
        stat.st_nlink = 1;
        stat.st_uid = 0;
        stat.st_gid = 0;
        stat.st_rdev = 0;
        stat.st_size = self.inner.content.lock().len() as i64;
        stat.st_blksize = 512;
        stat.st_blocks = 0;
        // stat.st_atime_sec = self.inner.times.lock()[1].tv_sec;
        // stat.st_atime_nsec = self.inner.times.lock()[1].tv_nsec;
        // stat.st_mtime_sec = self.inner.times.lock()[2].tv_sec;
        // stat.st_mtime_nsec = self.inner.times.lock()[2].tv_nsec;
        // stat.st_ctime_sec = self.inner.times.lock()[0].tv_sec;
        // stat.st_ctime_nsec = self.inner.times.lock()[0].tv_nsec;
        stat.st_atime_sec = self.inner.times.lock().access_time as i64;
        stat.st_atime_nsec = self.inner.times.lock().access_time as i64;
        stat.st_mtime_sec = self.inner.times.lock().modify_time as i64;
        Ok(())
    }

    // fn utimes(&self, times: &mut [TimeSpec]) -> VfsResult<()> {
    //     if times[0].tv_nsec != UTIME_OMIT {
    //         self.inner.times.lock()[1] = times[0]; // atime
    //     }
    //     if times[1].tv_nsec != UTIME_OMIT {
    //         self.inner.times.lock()[2] = times[1]; // mtime
    //     }
    //     Ok(())
    // }

    fn file_size(&self) -> usize {
        self.inner.content.lock().len()
    }

    fn offset(&self) -> usize {
        self.offset.lock().clone()
    }
}

impl File for RamFile {
    fn kernel_read_with_offset(&self, offset: usize, len: usize) -> Vec<u8> {
        let mut buffer = vec![0; len];
        assert!(offset + len <= self.file_size());
        let content = self.inner.content.lock();
        buffer.copy_from_slice(&content[offset..(offset + len)]);
        buffer
    }
    // TODO
    fn read_to_kspace(&self) -> Vec<u8> {
        let file_size = self.file_size();
        let offset = self.offset();
        assert!(offset <= file_size);
        let len = file_size - offset;
        let mut buffer = vec![0; len];
        let content = self.inner.content.lock();
        buffer.copy_from_slice(&content[offset..(offset + len)]);
        self.seek(file_size);
        buffer
    }
    // TODO
    fn read_to_ubuf(&self, mut buf: UserBuffer) -> usize {
        let offset = self.offset();
        self.pread(buf, offset)
    }
    fn pread(&self, mut buf: UserBuffer, offset: usize) -> usize {
        let mut offset = offset;
        let file_size = self.file_size();
        let mut total_read_size = 0usize;
        if file_size == 0 {
            if self.name() == "zero" || self.name() == "ZERO" {
                buf.write_zeros();
            }
            return 0;
        }
        if offset >= file_size {
            return 0;
        }
        let content = self.inner.content.lock();
        for sub_buff in buf.buffers.iter_mut() {
            let sub_buf_end = sub_buff.len();
            let offset = self.offset();
            let read_size = min(sub_buf_end, file_size - offset);
            sub_buff.copy_from_slice(&content[offset..offset + read_size]);
            self.seek(offset + read_size);
            total_read_size += read_size;
        }
        total_read_size
    }
    fn write_from_kspace(&self, buffer: &Vec<u8>) -> usize {
        let offset = self.offset();
        let file_size = self.file_size();
        let wsize = buffer.len();

        let part1 = cmp::min(file_size - offset, wsize);
        let mut content = self.inner.content.lock();
        content[offset..offset + part1].copy_from_slice(&&buffer[..part1]);
        // extend content if offset + buffer > content.len()
        content.extend_from_slice(&buffer[part1..]);

        self.seek(offset + wsize);
        wsize
    }
    fn pwrite(&self, buf: UserBuffer, offset: usize) -> usize {
        let mut offset = offset;
        let mut total_write_size = 0usize;
        let mut content = self.inner.content.lock();
        for sub_buf in buf.buffers.iter() {
            // let file_size = self.file_size(); // lock
            let file_size = content.len();
            let write_size = sub_buf.len();
            let part1 = min(write_size, file_size - offset);
            content[offset..offset + part1].copy_from_slice(&sub_buf[..part1]);
            content.extend_from_slice(&sub_buf[part1..]);
            total_write_size += write_size;
            offset += write_size;
            self.seek(offset);
        }
        total_write_size
    }
    fn write_from_ubuf(&self, buf: UserBuffer) -> usize {
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            self.file_size()
        } else {
            self.offset()
        };
        // let offset = self.file_size();
        self.pwrite(buf, offset)
    }
    // TODO
    fn set_time(&self, time_info: InodeTime) {
        let mut times = self.inner.times.lock();
        if times.modify_time < time_info.modify_time {
            times.access_time = time_info.access_time;
            times.create_time = time_info.create_time;
        } else {
            *times = time_info;
        }
    }
    // TODO
    fn dirent(&self, dirent: &mut Dirent) -> isize {
        -1
    }
    fn name(&self) -> String {
        self.inner.name()
    }
    fn fstat(&self, kstat: &mut Kstat) {
        self.stat(kstat).unwrap();
    }
    fn offset(&self) -> usize {
        self.offset()
    }
    fn seek(&self, pos: usize) {
        self.seek(pos);
    }
    fn flags(&self) -> OpenFlags {
        *self.inner.flags.lock()
    }
    fn set_flags(&self, flag: OpenFlags) {
        *self.inner.flags.lock() = flag;
    }
    fn set_cloexec(&self) {
        let mut flags = self.inner.flags.lock();
        *flags |= OpenFlags::O_CLOEXEC;
    }
    fn path(&self) -> AbsolutePath {
        self.inner.dir_path.lock().clone()
    }
    fn file_size(&self) -> usize {
        self.file_size()
    }
    fn truncate(&self, new_length: usize) {
        self.truncate(new_length);
    }
    fn fid(&self) -> u64 {
        self.mi.fs_id as u64
    }
    fn available(&self) -> bool {
        true
    }
    fn readable(&self) -> bool {
        // let rw = *self.inner.rw.lock();
        let rw = self.inner.rw();
        rw == RWablity::ReadOnly || rw == RWablity::ReadWrite
    }
    fn writable(&self) -> bool {
        // let rw = *self.inner.rw.lock();
        let rw = self.inner.rw();
        rw == RWablity::WriteOnly || rw == RWablity::ReadWrite
    }
    fn read_at_direct(&self, _offset: usize, _len: usize) -> Vec<u8> {
        panic!("{} not implement read_at_direct", self.name());
    }
    fn write_from_direct(&self, _offset: usize, _data: &Vec<u8>) -> usize {
        panic!("{} not implement write_from_direct", self.name());
    }
    fn delete(&self) -> usize {
        let path = self.path();
        let parent = find_parent_dir(path.clone()).unwrap();
        parent.remove(self.name()).unwrap();
        0
    }
    fn rename(&self, new_path: AbsolutePath, _flag: OpenFlags) {
        let new_parent = find_parent_dir(new_path.clone()).unwrap();
        let old_parent = find_parent_dir(self.path()).unwrap();
        let inner = self.inner.clone();
        let old_name = self.name();
        let new_name = new_path.last();
        old_parent.remove(old_name).unwrap();
        inner.set_path(new_path);
        inner.rename(new_name);
        new_parent.add_from_container(FileContainer::File(inner));
    }
    fn is_dir(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub filename: String,
    pub inode: usize,
    pub file_type: FileType,
    pub size: usize,
    pub childrens: usize,
}

pub struct DirEntry {
    pub filename: String,
    pub len: usize,
    pub file_type: FileType,
}

impl DirEntry {
    pub fn name(&self) -> &str {
        &self.filename
    }
}

#[derive(Clone)]
pub struct MountedInfo {
    pub fs_id: usize,
    pub path: AbsolutePath,
}

pub type VfsResult<T> = core::result::Result<T, VfsError>;

#[derive(Debug, Clone, Copy)]
pub enum VfsError {
    NotLinkFile,
    NotDir,
    NotFile,
    NotSupported,
    FileNotFound,
    AlreadyExists,
    InvalidData,
    DirectoryNotEmpty,
    InvalidInput,
    StorageFull,
    UnexpectedEof,
    WriteZero,
    Io,
    Blocking,
    NoMountedPoint,
    NotAPipe,
}

#[derive(Clone, Debug)]
pub enum FileType {
    File,
    Directory,
}
