//! Kernel file system implementation for FAT32.
//!
//! Design considerations for [`Inode`]:
//! 1. Enhance lookup efficiency by utilizing InodeCache for file caching.
//! 2. Regarding the fields of file and page cache:
//!     - Due to time constraints after the first stage of the national competition,
//!       it was not feasible to redesign the FAT32 file system or implement tempfs within the kernel.
//!       As a result, the VirtFile provided by the FAT32 file system was adopted as the kernel's file manipulation object.
//!     - During the submission process of the first stage of the national competition,
//!       we discovered that our file system design was inadequate, resulting in very slow execution speed.
//!       After the first stage, we conducted our own analysis and addressed the issue of inefficient cluster chain lookup in the FAT32 library.
//!       However, we were still troubled by the efficiency problems caused by direct disk/SD card read/write operations.
//!       That's when we came across TitanixOS, which was developed by contestants of the same session.
//!     - We greatly appreciate the design of TitanixOS. Its file and file system structure, as well as its functional design,
//!       are exceptionally excellent. In comparison, our kernel file design appears somewhat rudimentary:
//!       In our kernel, the files actually encapsulate the VirtFile provided by FAT32 into KFile,
//!       which results in data being synchronized to the disk every time it is written.
//!       However, after studying TitanixOS's PageCache design, we introduced a page caching mechanism for kernel files,
//!       effectively creating a virtual tempfs and significantly improving execution efficiency.
//! 3. Regarding the file_size field (storing the file size in the Inode):
//!     - During kernel execution, files created are often memory-mapped, treating them as files managed by a virtual tempfs.
//!     - The read and write operations on these files created during kernel execution are actually performed in memory using the Page Cache
//!       and are often not directly written back to the file system.
//!       This is because a large number of direct disk writes in a single-core environment would significantly slow down the kernel's execution speed.
//!     - Since the file_size parameter is required during file read and write operations and the files are not directly
//!       written back to the file system after each write or file close, retrieving the file size from the file system (inconsistently) is not feasible.
//!     - As different processes may write to the file, altering its size, when reopening the file with the Inode Cache,
//!       it is essential to ensure consistency in file size.

#[cfg(not(feature = "no-page-cache"))]
mod feature_no_page_cache {
    pub use crate::consts::PAGE_SIZE;
    pub use crate::fs::PageCache;
    pub use alloc::collections::BTreeMap;
    pub use spin::RwLock;
}
#[cfg(not(feature = "no-page-cache"))]
use feature_no_page_cache::*;

use crate::fs::open;
use crate::fs::{File, OpenFlags};
use crate::mm::UserBuffer;
use alloc::{string::String, sync::Arc, vec::Vec};
use fat32::VirtFile;

use nix::{CreateMode, Dirent, InodeTime, Kstat, S_IFCHR, S_IFDIR, S_IFREG};
use path::AbsolutePath;
use spin::lazy::Lazy;
use spin::{Mutex, MutexGuard};

#[cfg(not(feature = "no-page-cache"))]
pub const INODE_CACHE_LIMIT: usize = 1024;

/// InodeCache is used to cache the Inode of the file. Mainly used for the Open syscall.
#[cfg(all(not(feature = "no-page-cache"), not(feature = "hash-inode-cache")))]
pub struct InodeCache(pub RwLock<BTreeMap<AbsolutePath, Arc<Inode>>>);
// pub struct InodeCache(pub SpinLock<BTreeMap<AbsolutePath, Arc<Inode>>>);
#[cfg(all(not(feature = "no-page-cache"), feature = "hash-inode-cache"))]
pub struct InodeCache(pub RwLock<hashbrown::HashMap<AbsolutePath, Arc<Inode>>>);
#[cfg(all(not(feature = "no-page-cache"), not(feature = "hash-inode-cache")))]
// pub static INODE_CACHE: InodeCache = InodeCache(RwLock::new(BTreeMap::new()));
pub static INODE_CACHE: Lazy<InodeCache> = Lazy::new(|| {
    // println!("open test 0.4");
    // let ret = InodeCache(SpinLock::new(BTreeMap::new()));
    // println!("open test 0.5");
    // ret
    InodeCache(RwLock::new(BTreeMap::new()))
});

#[cfg(all(not(feature = "no-page-cache"), feature = "hash-inode-cache"))]
// lazy_static! {
// pub static ref INODE_CACHE: InodeCache = InodeCache(RwLock::new(hashbrown::HashMap::new()));
// }
pub static INODE_CACHE: Lazy<InodeCache> =
    Lazy::new(|| InodeCache(RwLock::new(hashbrown::HashMap::new())));

#[cfg(not(feature = "no-page-cache"))]
#[allow(unused)]
impl InodeCache {
    pub fn get(&self, path: &AbsolutePath) -> Option<Arc<Inode>> {
        self.0.read().get(path).cloned()
        // self.0.lock().get(path).cloned()
    }
    pub fn insert(&self, path: AbsolutePath, inode: Arc<Inode>) {
        self.0.write().insert(path, inode);
        if self.0.read().len() > INODE_CACHE_LIMIT {
            self.shrink();
        }
        // self.0.lock().insert(path, inode);
        // if self.0.lock().len() > INODE_CACHE_LIMIT {
        //     self.shrink();
        // }
    }
    pub fn remove(&self, path: &AbsolutePath) {
        self.0.write().remove(path);
        // self.0.lock().remove(path);
    }
    pub fn release(&self) {
        self.0.write().clear();
        // self.0.lock().clear();
    }
    pub fn shrink(&self) {
        // remove the item whose Inode strong reference count is 1
        let mut map = self.0.write();
        // let mut map = self.0.lock();
        let mut remove_list = Vec::new();
        for (path, inode) in map.iter() {
            if Arc::strong_count(inode) == 1 {
                remove_list.push(path.clone());
            }
        }
        for path in remove_list {
            map.remove(&path);
        }
    }
}

/// Kernel File
pub struct KFile {
    // read only feilds
    readable: bool,
    writable: bool,
    path: AbsolutePath, // It contains the file name, so the name field is not needed actually.
    name: String,

    // shared by some files (uaually happens when fork)
    pub time_info: Mutex<InodeTime>,
    pub offset: Mutex<usize>,
    pub flags: Mutex<OpenFlags>,
    pub available: Mutex<bool>,

    // shared by the same file (with page cache)
    pub inode: Arc<Inode>,
}

// You can see the introduction at the beginning of this file.
pub struct Inode {
    pub file: Mutex<Arc<VirtFile>>,
    pub fid: u64,
    #[cfg(not(feature = "no-page-cache"))]
    pub page_cache: Mutex<Option<Arc<PageCache>>>,
    #[cfg(not(feature = "no-page-cache"))]
    pub file_size: Mutex<usize>,
}

#[cfg(feature = "inode-drop")]
impl Drop for Inode {
    // Actually, all the tests create files in memory, read and write files,
    // and do not need to be written back to the file system.
    // TODO 实现 ramfs 将 page cache 转移到 ramfs
    fn drop(&mut self) {
        self.page_cache.lock().as_mut().unwrap().sync().unwrap();
    }
}

impl KFile {
    pub fn new(
        readable: bool,
        writable: bool,
        inode: Arc<Inode>,
        path: AbsolutePath,
        name: String,
    ) -> Self {
        let available = true;
        Self {
            readable,
            writable,
            path,
            name,
            inode,
            offset: Mutex::new(0),
            flags: Mutex::new(OpenFlags::empty()),
            available: Mutex::new(available),
            time_info: Mutex::new(InodeTime::empty()),
        }
    }
    pub fn file(&self) -> MutexGuard<'_, Arc<VirtFile>> {
        self.inode.file.lock()
    }
    #[cfg(not(feature = "no-page-cache"))]
    pub fn page_cache(&self) -> MutexGuard<'_, Option<Arc<PageCache>>> {
        self.inode.page_cache.lock()
    }
    #[cfg(not(feature = "no-page-cache"))]
    // Because of the weak pointer, we need to create a page cache after creating KFile.
    pub fn create_page_cache_if_needed(self: &Arc<Self>) {
        let mut page_cache = self.page_cache();
        if page_cache.is_none() {
            *page_cache = Some(Arc::new(PageCache::new(self.file().clone())));
        }
    }
    #[cfg(not(feature = "no-page-cache"))]
    pub fn write_all(&self, data: &Vec<u8>) -> usize {
        let mut total_write_size = 0usize;
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            self.file_size()
        } else {
            self.offset()
        };
        let mut slice_offset = 0;
        let slice_end = data.len();
        while slice_offset < slice_end {
            // to avoid slice's length spread page boundary
            let page = page_cache.get_page(offset, None).expect("get page error");
            let page_offset = offset % PAGE_SIZE;
            let mut slice_offset_end = slice_offset + (PAGE_SIZE - page_offset);
            if slice_offset_end > slice_end {
                slice_offset_end = slice_end;
            }
            let write_size = page
                .write(page_offset, &data[slice_offset..slice_offset_end])
                .expect("read page error");
            offset += write_size;
            self.seek(offset);
            slice_offset += write_size;
            total_write_size += write_size;
        }
        if self.file_size() < offset {
            self.set_file_size(offset);
        }
        total_write_size
    }
    #[cfg(feature = "no-page-cache")]
    pub fn write_all(&self, data: &Vec<u8>) -> usize {
        let file = self.file();
        let mut remain = data.len();
        let mut index = 0;
        loop {
            let len = remain.min(512);
            let offset = self.offset();
            file.write_at(offset, &data.as_slice()[index..index + len]);
            self.seek(offset + len);
            index += len;
            remain -= len;
            if remain == 0 {
                break;
            }
        }
        index
    }
    pub fn is_dir(&self) -> bool {
        let file = self.file();
        file.is_dir()
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn delete(&self) -> usize {
        let file = self.file();
        #[cfg(not(feature = "no-page-cache"))]
        let path = self.path.clone();
        #[cfg(not(feature = "no-page-cache"))]
        INODE_CACHE.remove(&path);
        file.clear()
    }
    pub fn delete_direntry(&self) {
        let file = self.file();
        file.clear_direntry();
    }
    #[cfg(not(feature = "no-page-cache"))]
    pub fn file_size(&self) -> usize {
        *self.inode.file_size.lock()
    }
    #[cfg(feature = "no-page-cache")]
    pub fn file_size(&self) -> usize {
        let file = self.file();
        file.file_size()
    }
    #[cfg(not(feature = "no-page-cache"))]
    pub fn set_file_size(&self, file_size: usize) {
        *self.inode.file_size.lock() = file_size;
    }
    #[cfg(feature = "no-page-cache")]
    pub fn set_file_size(&self, file_size: usize) {
        let file = self.file();
        file.set_file_size(file_size);
    }
    pub fn rename(&self, new_path: AbsolutePath, flags: OpenFlags) {
        // duplicate a new file, and set file cluster and file size
        let inner = self.file();
        // check file exits
        let new_file = open(new_path, flags, CreateMode::empty()).unwrap();
        let new_inner = new_file.file();
        let first_cluster = inner.first_cluster();
        let file_size = inner.file_size();
        new_inner.set_first_cluster(first_cluster);
        new_inner.set_file_size(file_size);
        drop(inner);
        // clear old direntry
        self.delete_direntry();
    }
    pub fn fid(&self) -> u64 {
        self.inode.fid
    }
}

impl File for KFile {
    //  No change file offset
    #[cfg(not(feature = "no-page-cache"))]
    fn kernel_read_with_offset(&self, offset: usize, len: usize) -> Vec<u8> {
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        let mut offset = offset;
        let mut buf: Vec<u8> = vec![0; len];
        let mut buf_offset = 0;
        let buf_end = len;

        while buf_offset < buf_end {
            let page = page_cache.get_page(offset, None).expect("get page error");
            let page_offset = offset % PAGE_SIZE;
            let mut buf_offset_end = buf_offset + (PAGE_SIZE - page_offset);
            if buf_offset_end > buf_end {
                buf_offset_end = buf_end;
            }
            let slice = buf.as_mut_slice();
            let read_size = page
                .read(page_offset, &mut slice[buf_offset..buf_offset_end])
                .expect("read page error");
            offset += read_size;
            buf_offset += read_size;
        }

        buf
    }
    #[cfg(feature = "no-page-cache")]
    fn kernel_read_with_offset(&self, offset: usize, len: usize) -> Vec<u8> {
        let file = self.file();
        let mut len = len;
        let mut offset = offset;
        let mut buffer = [0u8; 512];
        let mut ret: Vec<u8> = Vec::new();
        if len >= 96 * 4096 {
            // avoid ret's capacity too large
            ret.reserve(96 * 4096);
        }
        loop {
            let read_size = file.read_at(offset, &mut buffer);
            if read_size == 0 {
                break;
            }
            offset += read_size;
            if len > read_size {
                len -= read_size;
                ret.extend_from_slice(&buffer[..read_size]);
            } else {
                ret.extend_from_slice(&buffer[..len]);
                break;
            }
        }
        ret
    }
    // change file offset
    fn read_to_kspace(&self) -> Vec<u8> {
        let file_size = self.file_size();
        let offset = self.offset();
        let len = file_size - offset;
        let res = self.kernel_read_with_offset(offset, len);
        self.seek(offset + res.len());
        res
    }
    #[cfg(not(feature = "no-page-cache"))]
    fn read_to_ubuf(&self, mut buf: UserBuffer) -> usize {
        // with page cache
        #[cfg(feature = "time_trace")]
        time_trace!("read");
        let offset = self.offset();
        let file_size = self.file_size();
        let mut total_read_size = 0usize;
        if file_size == 0 {
            if self.name == "zero" {
                buf.write_zeros();
            }
            return 0;
        }
        if offset >= file_size {
            return 0;
        }
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        for slice in buf.buffers.iter_mut() {
            let slice_end = slice.len();
            let mut slice_offset = 0;
            while slice_offset < slice_end {
                // to avoid slice's length spread page boundary
                let offset = self.offset();
                let page = page_cache.get_page(offset, None).expect("get page error");
                let page_offset = offset % PAGE_SIZE;
                let mut slice_offset_end = slice_offset + (PAGE_SIZE - page_offset);
                if slice_offset_end > slice_end {
                    slice_offset_end = slice_end;
                }
                let read_size = page
                    .read(page_offset, &mut slice[slice_offset..slice_offset_end])
                    .expect("read page error");
                self.seek(offset + read_size);
                slice_offset += read_size;
                total_read_size += read_size;
            }
        }
        total_read_size
    }
    #[cfg(feature = "no-page-cache")]
    fn read_to_ubuf(&self, mut buf: UserBuffer) -> usize {
        #[cfg(feature = "time_trace")]
        time_trace!("read");
        let offset = self.offset();
        let file_size = self.file_size();
        let file = self.file();
        let mut total_read_size = 0usize;

        if file_size == 0 {
            if self.name == "zero" {
                buf.write_zeros();
            }
            return 0;
        }
        if offset >= file_size {
            return 0;
        }

        for slice in buf.buffers.iter_mut() {
            let read_size = file.read_at(offset, *slice);
            if read_size == 0 {
                break;
            }
            self.seek(offset + read_size);
            total_read_size += read_size;
        }
        total_read_size
    }
    // The same as read_to_ubuf, but will not change offset
    #[cfg(not(feature = "no-page-cache"))]
    fn pread(&self, mut buf: UserBuffer, offset: usize) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("read");
        let mut offset = offset;
        let file_size = self.file_size();
        let mut total_read_size = 0usize;
        if file_size == 0 {
            if self.name == "zero" {
                buf.write_zeros();
            }
            return 0;
        }
        if offset >= file_size {
            return 0;
        }
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        for slice in buf.buffers.iter_mut() {
            let slice_end = slice.len();
            let mut slice_offset = 0;
            while slice_offset < slice_end {
                // to avoid slice's length spread page boundary
                let page = page_cache.get_page(offset, None).expect("get page error");
                let page_offset = offset % PAGE_SIZE;
                let mut slice_offset_end = slice_offset + (PAGE_SIZE - page_offset);
                if slice_offset_end > slice_end {
                    slice_offset_end = slice_end;
                }
                let read_size = page
                    .read(page_offset, &mut slice[slice_offset..slice_offset_end])
                    .expect("read page error");
                offset += read_size;
                slice_offset += read_size;
                total_read_size += read_size;
            }
        }
        total_read_size
    }
    #[cfg(feature = "no-page-cache")]
    fn pread(&self, mut buf: UserBuffer, mut offset: usize) -> usize {
        #[cfg(feature = "time_trace")]
        time_trace!("read");
        let file_size = self.file_size();
        let file = self.file();
        let mut total_read_size = 0usize;

        if file_size == 0 {
            if self.name == "zero" {
                buf.write_zeros();
            }
            return 0;
        }
        if offset >= file_size {
            return 0;
        }

        for slice in buf.buffers.iter_mut() {
            let read_size = file.read_at(offset, *slice);
            if read_size == 0 {
                break;
            }
            offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    #[cfg(not(feature = "no-page-cache"))]
    fn write_from_kspace(&self, data: &Vec<u8>) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("write");
        let mut total_write_size = 0usize;
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            self.file_size()
        } else {
            self.offset()
        };
        let mut slice_offset = 0;
        let slice_end = data.len();
        while slice_offset < slice_end {
            // to avoid slice's length spread page boundary
            let page = page_cache.get_page(offset, None).expect("get page error");
            let page_offset = offset % PAGE_SIZE;
            let mut slice_offset_end = slice_offset + (PAGE_SIZE - page_offset);
            if slice_offset_end > slice_end {
                slice_offset_end = slice_end;
            }
            let write_size = page
                .write(page_offset, &data[slice_offset..slice_offset_end])
                .expect("read page error");
            offset += write_size;
            self.seek(offset);
            slice_offset += write_size;
            total_write_size += write_size;
        }
        if self.file_size() < offset {
            self.set_file_size(offset);
        }
        total_write_size
    }
    #[cfg(feature = "no-page-cache")]
    fn write_from_kspace(&self, data: &Vec<u8>) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("write");
        let file = self.file();
        let mut remain = data.len();
        let mut base = 0;
        loop {
            let len = remain.min(512);
            let offset = self.offset();
            file.write_at(offset, &data.as_slice()[base..base + len]);
            self.seek(offset + len);
            base += len;
            remain -= len;
            if remain == 0 {
                break;
            }
        }
        base
    }
    #[cfg(not(feature = "no-page-cache"))]
    fn write_from_ubuf(&self, buf: UserBuffer) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("write");
        let mut total_write_size = 0usize;
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            self.file_size()
        } else {
            self.offset()
        };
        for slice in buf.buffers.iter() {
            let slice_end = slice.len();
            let mut slice_offset = 0;
            while slice_offset < slice_end {
                // to avoid slice's length spread page boundary
                let page = page_cache.get_page(offset, None).expect("get page error");
                let page_offset = offset % PAGE_SIZE;
                let mut slice_offset_end = slice_offset + (PAGE_SIZE - page_offset);
                if slice_offset_end > slice_end {
                    slice_offset_end = slice_end;
                }
                let write_size = page
                    .write(page_offset, &slice[slice_offset..slice_offset_end])
                    .expect("read page error");
                offset += write_size;
                self.seek(offset);
                slice_offset += write_size;
                total_write_size += write_size;
            }
        }
        if self.file_size() < offset {
            self.set_file_size(offset);
        }
        total_write_size
    }
    #[cfg(feature = "no-page-cache")]
    fn write_from_ubuf(&self, buf: UserBuffer) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("write");
        let mut total_write_size = 0usize;
        let file_size = self.file_size();
        let file = self.file();
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            file_size
        } else {
            self.offset()
        };
        for slice in buf.buffers.iter() {
            let write_size = file.write_at(offset, *slice);
            assert_eq!(write_size, slice.len());
            offset += write_size;
            self.seek(offset);
            total_write_size += write_size;
        }
        total_write_size
    }
    #[cfg(not(feature = "no-page-cache"))]
    // The same as write_from_ubuf, but will not change offset
    fn pwrite(&self, buf: UserBuffer, offset: usize) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("write");
        let mut total_write_size = 0usize;
        let page_cache = self.page_cache().as_ref().cloned().unwrap();
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            self.file_size()
        } else {
            offset
        };
        for slice in buf.buffers.iter() {
            let slice_end = slice.len();
            let mut slice_offset = 0;
            while slice_offset < slice_end {
                // to avoid slice's length spread page boundary (howerver, it's low probability)
                let page = page_cache.get_page(offset, None).expect("get page error");
                let page_offset = offset % PAGE_SIZE;
                let mut slice_offset_end = slice_offset + (PAGE_SIZE - page_offset);
                if slice_offset_end > slice_end {
                    slice_offset_end = slice_end;
                }
                let write_size = page
                    .write(page_offset, &slice[slice_offset..slice_offset_end])
                    .expect("read page error");
                offset += write_size;
                slice_offset += write_size;
                total_write_size += write_size;
            }
        }
        if self.file_size() < offset {
            self.set_file_size(offset);
        }
        total_write_size
    }
    #[cfg(feature = "no-page-cache")]
    fn pwrite(&self, buf: UserBuffer, offset: usize) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("write");
        let mut total_write_size = 0usize;
        let file_size = self.file_size();
        let file = self.file();
        let mut offset = if self.flags().contains(OpenFlags::O_APPEND) {
            file_size
        } else {
            offset
        };
        for slice in buf.buffers.iter() {
            let write_size = file.write_at(offset, *slice);
            assert_eq!(write_size, slice.len());
            offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
    // TODO
    fn set_time(&self, time_info: InodeTime) {
        let mut time_lock = self.time_info.lock();
        // 根据测例改动
        if time_info.modify_time < time_lock.modify_time {
            time_lock.access_time = time_info.access_time;
            time_lock.create_time = time_info.create_time;
        } else {
            *time_lock = time_info;
        }
    }
    // set dir entry
    fn dirent(&self, dirent: &mut Dirent) -> isize {
        if !self.is_dir() {
            return -1;
        }
        let inner = self.file();
        let offset = self.offset();
        if let Some((name, offset, first_cluster, _attr)) = inner.dir_info(offset) {
            dirent.init(name.as_str(), offset as isize, first_cluster as usize);
            self.seek(offset as usize);
            // return size of Dirent as read size
            core::mem::size_of::<Dirent>() as isize
        } else {
            -1
        }
    }
    fn fstat(&self, kstat: &mut Kstat) {
        let name = self.name();
        let inner = self.file();
        let vfile = inner.clone();
        let mut st_mode = 0;
        _ = st_mode;
        #[cfg(not(feature = "no-page-cache"))]
        let (_, st_blksize, st_blocks, is_dir, _time) = vfile.stat();
        #[cfg(not(feature = "no-page-cache"))]
        let st_size = self.file_size();
        #[cfg(feature = "no-page-cache")]
        let (st_size, st_blksize, st_blocks, is_dir, _time) = vfile.stat();

        if is_dir {
            st_mode = S_IFDIR;
        } else {
            st_mode = S_IFREG;
        }
        // if &vfile.name() == "null"
        // || &vfile.name() == "NULL"
        // || &vfile.name() == "zero"
        // || &vfile.name() == "ZERO"
        if &name == "null" || &name == "NULL" || &name == "zero" || &name == "ZERO" {
            st_mode = S_IFCHR;
        }
        let time_info = self.time_info.lock();
        let atime = time_info.access_time;
        let mtime = time_info.modify_time;
        let ctime = time_info.create_time;
        let ino = self.fid();
        kstat.init(
            st_size as i64,
            st_blksize as i32,
            st_blocks as u64,
            ino,
            st_mode as u32,
            atime as i64,
            mtime as i64,
            ctime as i64,
        );
    }
    fn name(&self) -> String {
        self.name()
    }
    fn offset(&self) -> usize {
        *self.offset.lock()
    }
    fn seek(&self, offset: usize) {
        *self.offset.lock() = offset;
    }
    fn flags(&self) -> OpenFlags {
        *self.flags.lock()
    }
    fn set_flags(&self, flag: OpenFlags) {
        self.flags.lock().set(flag, true);
    }
    fn set_cloexec(&self) {
        *self.available.lock() = false;
    }
    fn path(&self) -> AbsolutePath {
        self.path.clone()
    }
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn available(&self) -> bool {
        *self.available.lock()
    }
    fn file_size(&self) -> usize {
        self.file_size()
    }
    fn truncate(&self, new_length: usize) {
        let inner = self.file();
        inner.modify_size(new_length);
    }
    fn fid(&self) -> u64 {
        self.fid()
    }
    // Currently not used in the kernel. Design problem, it can be used to design general
    // Inode and PageCache, which can use this method to create page cache
    // (the file parameter field of Inode and PageCache can be Arc<dyn File>,
    // but the current kernel file is coupled with fat32 file),
    fn read_at_direct(&self, offset: usize, len: usize) -> Vec<u8> {
        let mut buf: Vec<u8> = vec![0; len];
        let inner = self.file();
        inner.read_at(offset, &mut buf);
        buf
    }
    // Currently not used in the kernel. Design problem, it can be used to design general
    // Inode and PageCache, which can use this method to create page cache
    // (the file parameter field of Inode and PageCache can be Arc<dyn File>,
    // but the current kernel file is coupled with fat32 file),
    fn write_from_direct(&self, offset: usize, data: &Vec<u8>) -> usize {
        let inner = self.file();
        if offset + data.len() > self.file_size() {
            self.set_file_size(offset + data.len());
        }
        inner.write_at(offset, data)
    }
    fn delete(&self) -> usize {
        self.delete()
    }
    fn rename(&self, new_path: AbsolutePath, flags: OpenFlags) {
        self.rename(new_path, flags);
    }
    fn is_dir(&self) -> bool {
        self.is_dir()
    }
}
