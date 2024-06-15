pub const AT_FDCWD: isize = -100;

pub const TCGETS: usize = 0x5401;
pub const TCSETS: usize = 0x5402;
pub const TIOCGPGRP: usize = 0x540f;
pub const TIOCSPGRP: usize = 0x5410;
pub const TIOCGWINSZ: usize = 0x5413;
pub const RTC_RD_TIME: usize = 0xffffffff80247009; // 这个值还需考量

bitflags! {
#[derive(PartialEq, Eq, Debug)]
    pub struct FcntlFlags:usize{
        const F_DUPFD = 0;
        const F_GETFD = 1;
        const F_SETFD = 2;
        const F_GETFL = 3;
        const F_SETFL = 4;
        const F_GETLK = 5;
        const F_SETLK = 6;
        const F_SETLKW = 7;
        const F_SETOWN = 8;
        const F_GETOWN = 9;
        const F_SETSIG = 10;
        const F_GETSIG = 11;
        const F_SETOWN_EX = 15;
        const F_GETOWN_EX = 16;
        const F_GETOWNER_UIDS = 17;

        // 发现 F_UNLCK = 2 , 这个标记分类待研究
        const F_DUPFD_CLOEXEC = 1030;
    }
}

bitflags! {
    #[derive(Debug)]
    pub struct StatMode: u32 {
        const NULL  = 0;
        /// Type
        const TYPE_MASK = 0o170000;
        /// FIFO
        const FIFO  = 0o010000;
        /// character device
        const CHAR  = 0o020000;
        /// directory
        const DIR   = 0o040000;
        /// block device
        const BLOCK = 0o060000;
        /// ordinary regular file
        const FILE  = 0o100000;
        /// symbolic link
        const LINK  = 0o120000;
        /// socket
        const SOCKET = 0o140000;

        /// Set-user-ID on execution.
        const SET_UID = 0o4000;
        /// Set-group-ID on execution.
        const SET_GID = 0o2000;

        /// Read, write, execute/search by owner.
        const OWNER_MASK = 0o700;
        /// Read permission, owner.
        const OWNER_READ = 0o400;
        /// Write permission, owner.
        const OWNER_WRITE = 0o200;
        /// Execute/search permission, owner.
        const OWNER_EXEC = 0o100;

        /// Read, write, execute/search by group.
        const GROUP_MASK = 0o70;
        /// Read permission, group.
        const GROUP_READ = 0o40;
        /// Write permission, group.
        const GROUP_WRITE = 0o20;
        /// Execute/search permission, group.
        const GROUP_EXEC = 0o10;

        /// Read, write, execute/search by others.
        const OTHER_MASK = 0o7;
        /// Read permission, others.
        const OTHER_READ = 0o4;
        /// Write permission, others.
        const OTHER_WRITE = 0o2;
        /// Execute/search permission, others.
        const OTHER_EXEC = 0o1;
    }
}

pub const UTIME_NOW: u64 = 0x3fffffff;
pub const UTIME_OMIT: u64 = 0x3ffffffe;

bitflags! {
    #[derive(PartialEq, Eq)]
    pub struct SeekFlags: usize {
        const SEEK_SET = 0;   // 参数 offset 即为新的读写位置
        const SEEK_CUR = 1;   // 以目前的读写位置往后增加 offset 个位移量
        const SEEK_END = 2;   // 将读写位置指向文件尾后再增加 offset 个位移量
    }
}

#[repr(C)]
pub struct Statfs {
    pub f_type: u64,
    pub f_bsize: u64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: u64,
    pub f_namelen: u64,
    pub f_frsize: u64,
    pub f_flag: u64,
    pub f_spare: [u64; 4],
}
impl Statfs {
    // TODO
    pub fn new() -> Self {
        Self {
            f_type: 1,
            f_bsize: 512,
            f_blocks: 12345,
            f_bfree: 1234,
            f_bavail: 123,
            f_files: 1000,
            f_ffree: 100,
            f_fsid: 1,
            f_namelen: 123,
            f_frsize: 4096,
            f_flag: 123,
            f_spare: [0; 4],
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, size) }
    }
}

pub const NAME_LIMIT: usize = 64;

/// File information stored in directory entries
#[repr(C)]
#[derive(Debug)]
pub struct Dirent {
    pub d_ino: usize,             // inode number
    pub d_off: isize,             // offset from 0 to next dirent
    pub d_reclen: u16,            // length of this dirent
    pub d_type: u8,               // file type
    pub d_name: [u8; NAME_LIMIT], // file name (null-terminated)
}
impl Dirent {
    pub fn new() -> Self {
        Self {
            d_ino: 0,
            d_off: 0,
            d_reclen: core::mem::size_of::<Self>() as u16,
            d_type: 0,
            d_name: [0; NAME_LIMIT],
        }
    }
    pub fn init(&mut self, name: &str, offset: isize, first_clu: usize) {
        self.d_ino = first_clu;
        self.d_off = offset;
        self.fill_name(name);
    }
    fn fill_name(&mut self, name: &str) {
        let len = name.len().min(NAME_LIMIT);
        let name_bytes = name.as_bytes();
        for i in 0..len {
            self.d_name[i] = name_bytes[i];
        }
        self.d_name[len] = 0;
    }
    pub fn as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, size) }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct OpenFlags: u32 {
        // 来源不明
        // const O_RDONLY    = 0;
        // const O_WRONLY    = 1 << 0;
        // const O_RDWR      = 1 << 1;
        // const O_CREATE    = 1 << 6;
        // const O_EXCL      = 1 << 7;
        // const O_TRUNC     = 1 << 9;
        // const O_APPEND    = 1 << 10;
        // const O_NONBLOCK  = 1 << 11;
        // const O_LARGEFILE = 1 << 15;
        // const O_DIRECTORY = 1 << 16;
        // const O_NOFOLLOW  = 1 << 17;
        // const O_CLOEXEC   = 1 << 19;
        // 来源不明
        const O_ACCMODE =  0x0007;
        const O_EXEC    =  1;
        const O_RDONLY  =  2;
        const O_RDWR    =  3;
        const O_SEARCH  =  4;
        const O_WRONLY  =  5;

        // https://github.com/riscvarchive/riscv-musl/blob/staging/arch/riscv64/bits/fcntl.h
        const O_CREAT       =  0o100;
        const O_EXCL        =  0o200;
        const O_NOCTTY      =  0o400;
        const O_TRUNC       =  0o1000;
        const O_APPEND      =  0o2000;
        const O_NONBLOCK    =  0o4000;
        const O_DSYNC       =  0o10000;
        const O_SYNC        =  0o4010000;
        const O_RSYNC       =  0o4010000;
        const O_DIRECTORY   =  0o200000;
        const O_NOFOLLOW    =  0o400000;
        const O_CLOEXEC     =  0o2000000;

        const O_ASYNC       =  0o20000;
        const O_DIRECT      =  0o40000;
        const O_LARGEFILE   =  0o100000;
        const NOATIME     =  0o1000000;
        const PATH        =  0o10000000;
        const TMPFILE     =  0o20200000;
        const NDELAY      =  Self::O_NONBLOCK.bits();
    }

    /// User group read and write permissions
    #[derive(Debug)]
    pub struct CreateMode: u32 {
        const S_ISUID  = 0o4000;
        const S_ISGID  = 0o2000;
        const S_ISVTX  = 0o1000;

        const S_IRWXU  = 0o700;
        const S_IRUSR  = 0o400;
        const S_IWUSR  = 0o200;
        const S_IXUSR  = 0o100;

        const S_IRWXG  = 0o070;
        const S_IRGRP  = 0o040;
        const S_IWGRP  = 0o020;
        const S_IXGRP  = 0o010;

        const S_IRWXO  = 0o007;
        const S_IROTH  = 0o004;
        const S_IWOTH  = 0o002;
        const S_IXOTH  = 0o001;
    }
}
impl OpenFlags {
    // TODO
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::O_WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

use alloc::vec::Vec;

pub struct FdSet {
    fd_list: [u64; 16],
}

impl FdSet {
    pub fn new() -> Self {
        Self { fd_list: [0; 16] }
    }

    fn check_fd(fd: usize) -> bool {
        if fd < 1024 {
            return true;
        } else {
            return false;
        }
    }

    pub fn set_fd(&mut self, fd: usize) {
        if Self::check_fd(fd) {
            let index = fd >> 8; // fd/64
            let offset = fd - (index << 8); // fd%64
            self.fd_list[index] |= 1 << offset;
        }
    }

    pub fn clear_fd(&mut self, fd: usize) {
        // TODO wrong implement
        if Self::check_fd(fd) {
            let index = fd >> 8;
            let offset = fd - (index << 8);
            self.fd_list[index] &= (0 << offset) & 0xFFFFFFFFFFFFFFFF;
        }
    }

    pub fn clear_all(&mut self) {
        for i in 0..16 {
            self.fd_list[i] = 0;
        }
    }

    pub fn count(&mut self) -> usize {
        let fd_vec = self.get_fd_vec();
        fd_vec.len()
    }

    pub fn get_fd_vec(&self) -> Vec<usize> {
        let mut fd_v = Vec::new();
        for i in 0..16 {
            let mut tmp = self.fd_list[i];
            for off in 0..64 {
                let fd_bit = tmp & 1;
                if fd_bit == 1 {
                    fd_v.push((i << 8) + off); // index*64 + offset
                }
                tmp = tmp >> 1;
            }
        }
        fd_v
    }

    pub fn as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, size) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        let size = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as usize as *mut u8, size) }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PollFd {
    /// File descriptor
    pub fd: u32,
    /// Requested events
    pub events: PollEvent,
    /// Returned events
    pub revents: PollEvent,
}

bitflags! {
    /// Event types that can be polled for.
    ///
    /// These bits may be set in `events`(see `ppoll()`) to indicate the interesting event types;
    ///
    /// they will appear in `revents` to indicate the status of the file descriptor.
    #[derive(Debug, Clone, Copy)]
    pub struct PollEvent:u16 {
    /// There is data to read.
    const POLLIN = 0x001;
    /// There is urgent data to read.
    const POLLPRI = 0x002;
    /// Writing now will not block.
    const POLLOUT = 0x004;

    // These values are defined in XPG4.2.
    /// Normal data may be read.
    const POLLRDNORM = 0x040;
    /// Priority data may be read.
    const POLLRDBAND = 0x080;
    /// Writing now will not block.
    const POLLWRNORM = 0x100;
    /// Priority data may be written.
    const POLLWRBAND = 0x200;


    /// Linux Extension.
    const POLLMSG = 0x400;
    /// Linux Extension.
    const POLLREMOVE = 0x1000;
    /// Linux Extension.
    const POLLRDHUP = 0x2000;

    /* Event types always implicitly polled for.
    These bits need not be set in `events',
    but they will appear in `revents' to indicate the status of the file descriptor.*/

    /// Implicitly polled for only.
    /// Error condition.
    const POLLERR = 0x008;
    /// Implicitly polled for only.
    /// Hung up.
    const POLLHUP = 0x010;
    /// Implicitly polled for only.
    /// Invalid polling request.
    const POLLNVAL = 0x020;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InodeTime {
    pub create_time: u64,
    pub access_time: u64,
    pub modify_time: u64,
}
#[allow(unused)]
impl InodeTime {
    pub fn empty() -> Self {
        Self {
            access_time: 0,
            modify_time: 0,
            create_time: 0,
        }
    }
    /// Set the inode time's create time.
    pub fn set_create_time(&mut self, create_time: u64) {
        self.create_time = create_time;
    }
    /// Get a reference to the inode time's create time.
    pub fn create_time(&self) -> &u64 {
        &self.create_time
    }
    /// Set the inode time's access time.
    pub fn set_access_time(&mut self, access_time: u64) {
        self.access_time = access_time;
    }
    /// Get a reference to the inode time's access time.
    pub fn access_time(&self) -> &u64 {
        &self.access_time
    }
    /// Set the inode time's modify time.
    pub fn set_modify_time(&mut self, modify_time: u64) {
        self.modify_time = modify_time;
    }
    /// Get a reference to the inode time's modify time.
    pub fn modify_time(&self) -> &u64 {
        &self.modify_time
    }
}

pub const S_IFDIR: u32 = 0o0040000;
pub const S_IFCHR: u32 = 0o0020000;
pub const S_IFBLK: u32 = 0o0060000;
pub const S_IFREG: u32 = 0o0100000;
pub const S_IFIFO: u32 = 0o0010000;
pub const S_IFLNK: u32 = 0o0120000;
pub const S_IFSOCK: u32 = 0o0140000;

#[repr(C)]
#[derive(Debug)]
pub struct Kstat {
    pub st_dev: u64,   // 包含文件的设备 ID
    pub st_ino: u64,   // 索引节点号
    pub st_mode: u32,  // 文件类型和模式
    pub st_nlink: u32, // 硬链接数
    pub st_uid: u32,   // 所有者的用户 ID
    pub st_gid: u32,   // 所有者的组 ID
    pub st_rdev: u64,  // 设备 ID(如果是特殊文件)
    pub __pad: u64,
    pub st_size: i64,    // 总大小, 以字节为单位
    pub st_blksize: i32, // 文件系统 I/O 的块大小
    pub __pad2: i32,
    pub st_blocks: u64,     // 分配的 512B 块数
    pub st_atime_sec: i64,  // 上次访问时间
    pub st_atime_nsec: i64, // 上次访问时间(纳秒精度)
    pub st_mtime_sec: i64,  // 上次修改时间
    pub st_mtime_nsec: i64, // 上次修改时间(纳秒精度)
    pub st_ctime_sec: i64,  // 上次状态变化的时间
    pub st_ctime_nsec: i64, // 上次状态变化的时间(纳秒精度)
    pub __unused: [u32; 2],
}

impl Kstat {
    pub fn new() -> Self {
        Self {
            st_dev: 0,
            st_ino: 0 as u64,
            st_mode: 0,
            st_nlink: 0,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: 0,
            st_blksize: 0,
            __pad2: 0,
            st_blocks: 0,
            st_atime_sec: 0,
            st_atime_nsec: 0,
            st_mtime_sec: 0,
            st_mtime_nsec: 0,
            st_ctime_sec: 0,
            st_ctime_nsec: 0,
            __unused: [0; 2],
        }
    }

    pub fn init(
        &mut self,
        st_size: i64,
        st_blksize: i32,
        st_blocks: u64,
        st_ino: u64,
        st_mode: u32,
        st_atime_sec: i64,
        st_mtime_sec: i64,
        st_ctime_sec: i64,
    ) {
        self.st_nlink = 1;
        self.st_ino = st_ino;
        self.st_size = st_size;
        self.st_blksize = st_blksize;
        self.st_blocks = st_blocks;
        self.st_mode = st_mode;
        self.st_atime_sec = st_atime_sec;
        self.st_mtime_sec = st_mtime_sec;
        self.st_ctime_sec = st_ctime_sec;
    }

    pub fn as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<Self>();
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, size) }
    }
}
