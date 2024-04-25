//! FAT32 Directory Entry Structures

// #![allow(unused)]
use super::vf::VirtFileType;
use super::{
    ATTR_ARCHIVE, ATTR_DIRECTORY, ATTR_HIDDEN, ATTR_LONG_NAME, ATTR_READ_ONLY, ATTR_SYSTEM,
    ATTR_VOLUME_ID, DIR_ENTRY_LAST_AND_UNUSED, DIR_ENTRY_UNUSED, LAST_LONG_ENTRY,
    LONG_NAME_LEN_CAP, SPACE,
};

use alloc::string::{String, ToString};
use core::default::Default;
use core::iter::Iterator;
use core::option::Option::{None, Some};
use core::str;

/// FAT 32 Byte Directory Entry Structure
///
// 9 + 3 + 1 + 1 + 1 + 1 + 2 + 2 + 2 + 4 + 4 = 32 bytes
#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct ShortDirEntry {
    /// Short Name
    ///
    /// size: (8+3) bytes    offset: 0 (0x0~0xA)
    //
    //  文件名, 如果该目录项正在使用中 0x0 位置的值为文件名或子目录名的第一个字符, 如果该目录项未被使用
    //  name[0] 位置的值为 0x0, 如果该目录项曾经被使用过但是现在已经被删除则 name[0] 位置的值为 0xE5
    name: [u8; 8],
    /// Short Name Extension
    extension: [u8; 3],
    /// Attributes
    ///
    /// size: 1 byte      offset: 11 Bytes (0xB)
    //
    //  描述文件的属性, 该字段在短文件中不可取值 0x0F (标志是长文件)
    attr: u8,
    // attr: FATAttr,
    /// Reserved for Windows NT
    ///
    /// size: 1 byte      offset: 12 Bytes (0xC)    value: 0x00
    //
    //  这个位默认为 0, 只有短文件名时才有用. 一般初始化为 0 后不再修改, 可能的用法为:
    //  当为 0x00 时为文件名全大写, 当为 0x08 时为文件名全小写;
    //  0x10 时扩展名全大写, 0x00 扩展名全小写; 当为 0x18 时为文件名全小写, 扩展名全大写
    nt_res: u8,
    /// Millisecond stamp at file creation time. This field actually
    /// contains a count of tenths of a second. The granularity of the
    /// seconds part of DIR_CrtTime is 2 seconds so this field is a
    /// count of tenths of a second and its valid value range is 0-199
    /// inclusive.
    ///
    /// size: 1 byte      offset: 13 Bytes (0xD)    value range: 0-199
    _crt_time_tenth: u8,
    /// Time file was created
    /// The granularity of the seconds part of DIR_CrtTime is 2 seconds.
    ///
    /// size: 2 bytes     offset: 14 Bytes (0xE ~ 0xF)
    //
    //  文件创建的时间: 时-分-秒, 16bit 被划分为 3个部分:
    //    0~4bit 为秒, 以 2秒为单位, 有效值为 0~29, 可以表示的时刻为 0~58
    //    5~10bit 为分, 有效值为 0~59
    //    11~15bit 为时, 有效值为 0~23
    crt_time: u16,
    /// Date file was created
    ///
    /// size: 2 bytes     offset: 16 Bytes (0x10~0x11)
    //
    //  文件创建日期, 16bit 也划分为三个部分:
    //    0~4bit 为日, 有效值为 1~31
    //    5~8bit 为月, 有效值为 1~12
    //    9~15bit 为年, 有效值为 0~127, 这是一个相对于 1980 年的年数值 (该值加上 1980 即为文件创建的日期值 (1980–2107))
    crt_date: u16,
    /// Last access date
    ///
    /// size: 2 bytes     offset: 18 Bytes (0x12~0x13)
    lst_acc_date: u16,
    /// High word (16 bis) of this entry's first cluster number (always 0 on FAT12 and FAT16)
    ///
    /// size: 2 bytes     offset: 20 Bytes (0x14~0x15)
    fst_clus_hi: u16,
    /// Time of last write
    /// Note that file creation is considered a write.
    ///
    /// size: 2 bytes     offset: 22 Bytes (0x16~0x17)
    wrt_time: u16,
    /// Date of last write
    /// Note that file creation is considered a write.
    ///
    /// size: 2 bytes     offset: 24 Bytes (0x18~0x19)
    wrt_date: u16,
    /// Cluster number of the first cluster
    /// Low word (16-bit) of this entry's first cluster number
    ///
    /// size: 2 bytes     offset: 26 Bytes (0x1A~0x1B)
    //
    //  文件内容起始簇号的低两个字节, 与 0x14~0x15 字节处的高两个字节组成文件内容起始簇号
    fst_clus_lo: u16,
    /// File size in bytes
    /// size: 4 bytes     offset: 28 Bytes (0x1C~0x1F)
    file_size: u32,
}

impl Default for ShortDirEntry {
    fn default() -> Self {
        Self::empty()
    }
}

impl ShortDirEntry {
    pub fn empty() -> Self {
        Self {
            name: [0; 8],
            extension: [0; 3],
            attr: ATTR_ARCHIVE,
            nt_res: 0,
            _crt_time_tenth: 0,
            crt_time: 0,
            crt_date: 0,
            lst_acc_date: 0,
            fst_clus_hi: 0,
            wrt_time: 0,
            wrt_date: 0,
            fst_clus_lo: 0,
            file_size: 0,
        }
    }
    // All names must check if they have existed in the directory
    pub fn new(cluster: u32, name: &[u8], extension: &[u8], create_type: VirtFileType) -> Self {
        let mut item = Self::empty();
        let mut name_: [u8; 8] = [SPACE; 8];
        let mut extension_: [u8; 3] = [SPACE; 3];
        name_[0..name.len()].copy_from_slice(name);
        extension_[0..extension.len()].copy_from_slice(extension);

        name_[..].make_ascii_uppercase();
        extension_[..].make_ascii_uppercase();

        item.name = name_;
        item.extension = extension_;
        match create_type {
            VirtFileType::File => item.attr = ATTR_ARCHIVE,
            VirtFileType::Dir => item.attr = ATTR_DIRECTORY,
        }
        item.set_first_cluster(cluster);
        item
    }
    // All names must check if they have existed in the directory
    pub fn new_from_name_bytes(cluster: u32, name_bytes: &[u8], create_type: VirtFileType) -> Self {
        let mut item = [0; 32];
        item[0x00..0x0B].copy_from_slice(name_bytes);
        item[0x00..0x00 + name_bytes.len()].make_ascii_uppercase();
        let mut cluster: [u8; 4] = cluster.to_be_bytes();
        cluster.reverse();
        item[0x14..0x16].copy_from_slice(&cluster[2..4]);
        item[0x1A..0x1C].copy_from_slice(&cluster[0..2]);
        match create_type {
            VirtFileType::Dir => item[0x0B] = ATTR_DIRECTORY,
            VirtFileType::File => item[0x10] = ATTR_ARCHIVE,
        }
        unsafe { *(item.as_ptr() as *const ShortDirEntry) }
    }
    pub fn gen_check_sum(&self) -> u8 {
        let mut name_: [u8; 11] = [0u8; 11];
        let mut sum: u32 = 0;
        for i in 0..8 {
            name_[i] = self.name[i];
        }
        for i in 0..3 {
            name_[i + 8] = self.extension[i];
        }
        for i in 0..11 {
            sum = (((sum & 1) << 7) + (sum >> 1) + name_[i] as u32) & 0xFF;
        }
        sum as u8
    }
    pub fn name(&self) -> String {
        let name_len = self.name.iter().position(|&x| x == SPACE).unwrap_or(8);
        let ext_len = self.extension.iter().position(|&x| x == SPACE).unwrap_or(3);
        macro_rules! as_u8str {
            ($a:expr) => {
                core::str::from_utf8(&$a).unwrap_or("")
            };
        }
        {
            if ext_len != 0 {
                [
                    as_u8str!(self.name[..name_len]),
                    as_u8str!(['.' as u8][..]),
                    as_u8str!(self.extension[..ext_len]),
                ]
                .join("")
            } else {
                as_u8str!(self.name[0..name_len]).to_string()
            }
        }
    }
    // Get the start cluster number of the file
    pub fn first_cluster(&self) -> u32 {
        ((self.fst_clus_hi as u32) << 16) + (self.fst_clus_lo as u32)
    }
    // Set the start cluster number of the file
    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.fst_clus_hi = ((cluster & 0xFFFF0000) >> 16) as u16;
        self.fst_clus_lo = (cluster & 0x0000FFFF) as u16;
    }
    pub fn is_deleted(&self) -> bool {
        self.name[0] == DIR_ENTRY_UNUSED
    }
    pub fn is_empty(&self) -> bool {
        self.name[0] == DIR_ENTRY_LAST_AND_UNUSED
    }
    pub fn delete(&mut self) {
        self.file_size = 0;
        self.set_first_cluster(0);
        self.name[0] = DIR_ENTRY_UNUSED;
    }
    pub fn file_size(&self) -> u32 {
        self.file_size
    }
    pub fn set_file_size(&mut self, file_size: u32) {
        self.file_size = file_size;
    }
    pub fn is_dir(&self) -> bool {
        self.attr == ATTR_DIRECTORY
    }
    pub fn get_name_uppercase(&self) -> String {
        let mut name: String = String::new();
        for i in 0..8 {
            if self.name[i] == SPACE {
                break;
            } else {
                name.push(self.name[i] as char);
            }
        }
        for i in 0..3 {
            if self.extension[i] == SPACE {
                break;
            } else {
                if i == 0 {
                    name.push('.');
                }
                name.push(self.extension[i] as char);
            }
        }
        name
    }
    pub fn get_name_lowercase(&self) -> String {
        self.get_name_uppercase().to_ascii_lowercase()
    }
    pub fn set_name_case(&mut self, state: u8) {
        self.nt_res = state;
    }
    pub fn attr(&self) -> u8 {
        self.attr as u8
    }
    pub fn set_attr(&mut self, attr: u8) {
        self.attr = attr;
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const ShortDirEntry as *const u8, 32) }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut ShortDirEntry as *mut u8, 32) }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
/// Long Directory Entry
///
/// 1 + 2*5 + 1 + 1 + 2 + 2*6 + 2 + 2*2 = 32 bytes
pub struct LongDirEntry {
    /// Long Dir Entry Order   size: 1 byte    offset: 0 (0x00)
    //
    //  长文件名目录项的序列号, 一个文件的第一个目录项序列号为 1, 然后依次递增. 如果是该文件的
    //  最后一个长文件名目录项, 则将该目录项的序号与 0x40 进行 "或 (OR) 运算"的结果写入该位置.
    //  如果该长文件名目录项对应的文件或子目录被删除, 则将该字节设置成删除标志0xE5.
    //
    //  Mask(0x40)针对同一个文件中的 ord, 一个长目录项的长文件名仅有 13 个 unicode字符,
    //  当文件名超过13个字符时, 需要多个长目录项
    ord: u8,
    /// Characters 1-5 of the long-name sub-component in this dir entry.
    /// CharSet: Unicode. Codeing: UTF-16LE
    ///
    /// Long Dir Entry Name 1  size: 10 bytes  offset: 1 (0x01~0x0A)
    //
    //  长文件名的第 1~5 个字符. 长文件名使用 Unicode 码, 每个字符需要两个字节的空间.
    //  如果文件名结束但还有未使用的字节, 则会在文件名后先填充两个字节的 "00", 然后开始使用 0xFF 填充
    name1: [u16; 5],
    /// Attributes - must be ATTR_LONG_NAME
    ///
    /// Long Dir Entry Attributes   size: 1 byte    offset: 11 (0x0B)
    //
    //  长目录项的属性标志, 一定是 0x0F
    attr: u8,
    /// Long Dir Entry Type    size: 1 byte    offset: 12 (0x0C)   value: 0 (sub-component of long name)
    _ldir_type: u8,
    /// Checksum of name in the short dir entry at the end of the long dir set.
    ///
    /// Checksum      size: 1 byte    offset: 13 (0x0D)
    //
    //  校验和. 如果一个文件的长文件名需要几个长文件名目录项进行存储, 则这些长文件名目录项具有相同的校验和.
    chk_sum: u8,
    /// Characters 6-11 of the long-name sub-component in this dir entry.
    /// CharSet: Unicode. Codeing: UTF-16LE
    ///
    /// Long Dir Entry Name 2  size: 12 bytes  offset: 14 (0x0E~0x19)
    ///
    //  文件名的第 6~11 个字符, 未使用的字节用 0xFF 填充
    name2: [u16; 6],
    /// Long Dir Entry First Cluster Low   size: 2 bytes   offset: 26 (Ox1A~0x1B)     value: 0
    _fst_clus_lo: u16,
    /// Characters 12-13 of the long-name sub-component in this dir entry.
    /// CharSet: Unicode. Codeing: UTF-16LE
    ///
    /// Long Dir Entry Name 3  size: 4 bytes   offset: 28 (0x1C~0x1F)
    //
    //  文件名的第 12~13 个字符, 未使用的字节用 0xFF 填充
    name3: [u16; 2],
}

impl LongDirEntry {
    pub fn empty() -> Self {
        Self {
            ord: 0u8,
            name1: [0u16; 5],
            attr: ATTR_LONG_NAME,
            _ldir_type: 0u8,
            chk_sum: 0u8,
            name2: [0u16; 6],
            _fst_clus_lo: 0u16,
            name3: [0u16; 2],
        }
    }
    pub fn new_form_name_slice(order: u8, name_array: [u16; 13], check_sum: u8) -> Self {
        let mut lde = Self::empty();
        unsafe {
            core::ptr::addr_of_mut!(lde.name1)
                // try_into() 被用来尝试将 partial_name[..5] 转换成一个大小为 5 的固定大小数组
                .write_unaligned(name_array[..5].try_into().expect("Failed to cast!"));
            core::ptr::addr_of_mut!(lde.name2)
                .write_unaligned(name_array[5..11].try_into().expect("Failed to cast!"));
            core::ptr::addr_of_mut!(lde.name3)
                .write_unaligned(name_array[11..].try_into().expect("Failed to cast!"));
        }

        lde.ord = order;
        lde.chk_sum = check_sum;

        lde
    }
    pub fn name(&self) -> String {
        let name_all = self.name_utf16();
        let len = (0..name_all.len())
            .find(|i| name_all[*i] == 0)
            .unwrap_or(name_all.len());
        // 从 UTF-16 编码的字节数组中解码出字符串
        String::from_utf16_lossy(&name_all[..len])
    }
    pub fn name_utf16(&self) -> [u16; LONG_NAME_LEN_CAP] {
        let mut name_all: [u16; LONG_NAME_LEN_CAP] = [0u16; LONG_NAME_LEN_CAP];

        name_all[..5].copy_from_slice(unsafe { &core::ptr::addr_of!(self.name1).read_unaligned() });
        name_all[5..11]
            .copy_from_slice(unsafe { &core::ptr::addr_of!(self.name2).read_unaligned() });
        name_all[11..]
            .copy_from_slice(unsafe { &core::ptr::addr_of!(self.name3).read_unaligned() });

        name_all
    }
    pub fn attr(&self) -> u8 {
        self.attr
    }
    pub fn order(&self) -> u8 {
        self.ord
    }
    pub fn check_sum(&self) -> u8 {
        self.chk_sum
    }
    pub fn is_deleted(&self) -> bool {
        self.ord == DIR_ENTRY_UNUSED
    }
    pub fn delete(&mut self) {
        self.ord = DIR_ENTRY_UNUSED;
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const Self as *const u8, 32) }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut Self as *mut u8, 32) }
    }
}

// May Unused

#[allow(unused)]
pub(crate) enum NameType {
    SFN,
    LFN,
}

#[allow(unused)]
#[derive(PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum FATAttr {
    /// Indicates that writes to the file should fail.
    AttrReadOnly = ATTR_READ_ONLY,
    /// Indicates that normal directory listings should not show this file.
    AttrHidden = ATTR_HIDDEN,
    /// Indicates that this is an operating system file.
    AttrSystem = ATTR_SYSTEM,
    /// Root Dir
    AttrVolumeID = ATTR_VOLUME_ID,
    /// Indicates that this file is actually a container for other files.
    AttrDirectory = ATTR_DIRECTORY,
    /// This attribute supports backup utilities. This bit is set by the FAT file
    /// system driver when a file is created, renamed, or written to.
    AttrArchive = ATTR_ARCHIVE,
    /// Idicates that the "file" is actually part of the long name entry for some other file.
    AttrLongName = ATTR_LONG_NAME,
}

#[allow(unused)]
impl ShortDirEntry {
    pub fn name_bytes_array_with_dot(&self) -> ([u8; 12], usize) {
        let mut len = 0;
        let mut full_name = [0; 12];

        for &i in self.name.iter() {
            if i != SPACE {
                full_name[len] = i;
                len += 1;
            }
        }
        if self.extension[0] != SPACE {
            full_name[len] = b'.';
            len += 1;
        }
        for &i in self.extension.iter() {
            if i != SPACE {
                full_name[len] = i;
                len += 1;
            }
        }
        (full_name, len)
    }
    pub fn name_bytes_array(&self) -> [u8; 11] {
        let mut full_name = [0; 11];
        let mut len = 0;

        for &i in self.name.iter() {
            if i != SPACE {
                full_name[len] = i;
                len += 1;
            }
        }
        for &i in self.extension.iter() {
            if i != SPACE {
                full_name[len] = i;
                len += 1;
            }
        }
        full_name
    }
    // All names must check if they have existed in the directory
    pub fn new_form_name_str(cluster: u32, name_str: &str, create_type: VirtFileType) -> Self {
        let (name, extension) = match name_str.find('.') {
            Some(i) => (&name_str[0..i], &name_str[i + 1..]),
            None => (&name_str[0..], ""),
        };
        let mut item = [0; 32];
        let _item = [SPACE; 11];
        // 初始化为 0x20, 0x20 为 ASCII 码中的空格字符; 0x00..0x0B = 0..11
        item[0x00..0x0B].copy_from_slice(&_item);
        // name 的长度可能不足 8 个字节; 0..name.len()
        item[0x00..0x00 + name.len()].copy_from_slice(name.as_bytes());
        // ext 的长度可能不足 3 个字节; 8..extension.len()
        item[0x08..0x08 + extension.len()].copy_from_slice(extension.as_bytes());
        // 将 name 和 ext 部分转换为大写
        //
        // "Short names passed to the file system are always converted to upper case and their original case value is lost"
        //
        item[0x00..0x00 + name.len()].make_ascii_uppercase();
        item[0x08..0x08 + extension.len()].make_ascii_uppercase();
        // 采用小端序存储数据, 与 FAT32 文件系统的存储方式一致
        //
        // FAT file system on disk data structure is all "little endian".
        //
        // to_le_bytes() 方法将 u32 类型的数据转换为 小端序 的字节数组
        // eg. 0x12345678 -> [0x78, 0x56, 0x34, 0x12]
        let cluster: [u8; 4] = cluster.to_le_bytes();
        // 0x1A~0x1B 字节为文件内容起始簇号的低两个字节, 与 0x14~0x15 字节处的高两个字节组成文件内容起始簇号
        item[0x14..0x16].copy_from_slice(&cluster[2..4]);
        item[0x1A..0x1C].copy_from_slice(&cluster[0..2]);
        match create_type {
            VirtFileType::Dir => item[0x0B] = ATTR_DIRECTORY,
            VirtFileType::File => item[0x10] = ATTR_ARCHIVE,
        }
        unsafe { *(item.as_ptr() as *const ShortDirEntry) }
    }
    // All names must check if they have existed in the directory
    pub fn set_name(&mut self, name: &[u8], extension: &[u8]) {
        let mut name_: [u8; 8] = [SPACE; 8];
        name_[0..name.len()].make_ascii_uppercase();
        name_[0..name.len()].copy_from_slice(name);

        let mut extension_: [u8; 3] = [SPACE; 3];
        extension_[0..extension.len()].make_ascii_uppercase();
        extension_[0..extension.len()].copy_from_slice(extension);
        self.name = name_;
    }
    /// directory entry is free
    pub fn is_free(&self) -> bool {
        self.name[0] == DIR_ENTRY_UNUSED
            || self.name[0] == DIR_ENTRY_LAST_AND_UNUSED
            || self.name[0] == 0x05
    }
    // 见文件顶部的 Name[0] 说明
    pub fn is_valid_name(&self) -> bool {
        if self.name[0] < 0x20 {
            return self.name[0] == 0x05;
        } else {
            for i in 0..8 {
                if i < 3 {
                    if self.extension[i] < 0x20 {
                        return false;
                    }
                    if self.extension[i] == 0x22
                        || self.extension[i] == 0x2A
                        || self.extension[i] == 0x2E
                        || self.extension[i] == 0x2F
                        || self.extension[i] == 0x3A
                        || self.extension[i] == 0x3C
                        || self.extension[i] == 0x3E
                        || self.extension[i] == 0x3F
                        || self.extension[i] == 0x5B
                        || self.extension[i] == 0x5C
                        || self.extension[i] == 0x5D
                        || self.extension[i] == 0x7C
                    {
                        return false;
                    }
                }
                if self.name[i] < 0x20 {
                    return false;
                }
                if self.name[i] == 0x22
                    || self.name[i] == 0x2A
                    || self.name[i] == 0x2E
                    || self.name[i] == 0x2F
                    || self.name[i] == 0x3A
                    || self.name[i] == 0x3C
                    || self.name[i] == 0x3E
                    || self.name[i] == 0x3F
                    || self.name[i] == 0x5B
                    || self.name[i] == 0x5C
                    || self.name[i] == 0x5D
                    || self.name[i] == 0x7C
                {
                    return false;
                }
            }
            return true;
        }
    }
    pub fn is_file(&self) -> bool {
        self.attr == ATTR_ARCHIVE
            || self.attr == ATTR_HIDDEN
            || self.attr == ATTR_SYSTEM
            || self.attr == ATTR_READ_ONLY
    }
    pub fn as_bytes_array_mut(&mut self) -> &mut [u8; 32] {
        unsafe { &mut *(self as *mut ShortDirEntry as *mut [u8; 32]) }
    }
    pub fn to_bytes_array(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(self.as_bytes());
        bytes
    }
    pub fn as_bytes_array(&self) -> &[u8; 32] {
        unsafe { &*(self as *const ShortDirEntry as *const [u8; 32]) }
    }
    pub fn new_from_bytes(buf: &[u8]) -> Self {
        unsafe { *(buf.as_ptr() as *const ShortDirEntry) }
    }
}

#[allow(unused)]
impl ShortDirEntry {
    pub fn set_create_time(&mut self, time: u16) {
        self.crt_time = time;
    }
    pub fn set_create_date(&mut self, date: u16) {
        self.crt_date = date;
    }
    pub fn set_last_access_date(&mut self, date: u16) {
        self.lst_acc_date = date;
    }
    pub fn set_last_write_time(&mut self, time: u16) {
        self.wrt_time = time;
    }
    pub fn set_last_write_date(&mut self, date: u16) {
        self.wrt_date = date;
    }
}

#[allow(unused)]
impl LongDirEntry {
    pub fn set_name(&mut self, name_array: [u16; 13]) {
        unsafe {
            core::ptr::addr_of_mut!(self.name1)
                // try_into() 被用来尝试将 partial_name[..5] 转换成一个大小为 5 的固定大小数组
                .write_unaligned(name_array[..5].try_into().expect("Failed to cast!"));
            core::ptr::addr_of_mut!(self.name2)
                .write_unaligned(name_array[5..11].try_into().expect("Failed to cast!"));
            core::ptr::addr_of_mut!(self.name3)
                .write_unaligned(name_array[11..].try_into().expect("Failed to cast!"));
        }
    }
    pub fn new(order: u8, check_sum: u8, name_str: &str) -> Self {
        let mut buf = [0; 32];
        buf[0x00] = order;
        buf[0x0B] = ATTR_LONG_NAME;
        buf[0x0D] = check_sum;
        Self::write_unicode(name_str, &mut buf);
        Self::new_form_bytes(&buf)
    }
    pub fn new_form_bytes(buf: &[u8]) -> Self {
        unsafe { *(buf.as_ptr() as *const Self) }
    }
    pub fn is_free(&self) -> bool {
        self.ord == DIR_ENTRY_LAST_AND_UNUSED || self.ord == DIR_ENTRY_UNUSED
    }
    pub fn is_empty(&self) -> bool {
        self.ord == DIR_ENTRY_LAST_AND_UNUSED
    }
    pub fn is_valid(&self) -> bool {
        self.ord != DIR_ENTRY_UNUSED
    }
    fn write_unicode(value: &str, buf: &mut [u8]) {
        let mut temp = [0xFF; 26];
        let mut index = 0;

        for i in value.encode_utf16() {
            // u16 低 8 位
            let part1 = (i & 0xFF) as u8;
            // u16 高 8 位
            let part2 = ((i & 0xFF00) >> 8) as u8;
            temp[index] = part1;
            temp[index + 1] = part2;
            index += 2;
        }
        //  如果文件名结束但还有未使用的字节, 则会在文件名后先填充两个字节的 "00", 然后开始使用 0xFF 填充
        if index != 26 {
            temp[index] = 0;
            temp[index + 1] = 0;
        }
        index = 0;
        let mut option = |start: usize, end: usize| {
            for i in (start..end).step_by(2) {
                buf[i] = temp[index];
                buf[i + 1] = temp[index + 1];
                index += 2;
            }
        };
        option(0x01, 0x0A);
        option(0x0E, 0x19);
        option(0x1C, 0x1F);
    }
    fn name_to_utf8(&self) -> ([u8; 13 * 3], usize) {
        let (mut utf8, mut len) = ([0; 13 * 3], 0);
        let mut option = |parts: &[u16]| {
            for i in 0..parts.len() {
                let unicode: u16 = parts[i];
                if unicode == 0 || unicode == 0xFFFF {
                    break;
                }
                // UTF-16 转 UTF-8 编码
                // UTF-8 编码的规则:
                // 如果代码点在 0x80 以下 (即 ASCII 字符), 则使用 1 个字节的编码表示, 即 0xxxxxxx (其中 x 表示可用的位)
                // 如果代码点在 0x80 到 0x7FF 之间, 则使用 2 个字节的编码表示, 即 110xxxxx 10xxxxxx.
                // 如果代码点在 0x800 到 0xFFFF 之间, 则使用 3 个字节的编码表示, 即 1110xxxx 10xxxxxx 10xxxxxx
                // 如果代码点在 0x10000 到 0x10FFFF 之间, 则使用 4 个字节的编码表示, 即 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
                if unicode <= 0x007F {
                    utf8[len] = unicode as u8;
                    len += 1;
                } else if unicode >= 0x0080 && unicode <= 0x07FF {
                    let part1 = (0b11000000 | (0b00011111 & (unicode >> 6))) as u8;
                    let part2 = (0b10000000 | (0b00111111) & unicode) as u8;

                    utf8[len] = part1;
                    utf8[len + 1] = part2;
                    len += 2;
                } else if unicode >= 0x0800 {
                    let part1 = (0b11100000 | (0b00011111 & (unicode >> 12))) as u8;
                    let part2 = (0b10000000 | (0b00111111) & (unicode >> 6)) as u8;
                    let part3 = (0b10000000 | (0b00111111) & unicode) as u8;

                    utf8[len] = part1;
                    utf8[len + 1] = part2;
                    utf8[len + 2] = part3;
                    len += 3;
                }
            }
        };
        unsafe {
            option(&core::ptr::addr_of!(self.name1).read_unaligned());
            option(&core::ptr::addr_of!(self.name2).read_unaligned());
            option(&core::ptr::addr_of!(self.name3).read_unaligned());
        }

        (utf8, len)
    }

    // The mask should be for ord in the same file. The long
    // file name of a long directory entry only has 13 unicode
    // characters. When the file name exceeds 13 characters,
    // multiple long directory entries are required.
    pub fn lde_order(&self) -> usize {
        (self.ord & (LAST_LONG_ENTRY - 1)) as usize
    }
    pub fn is_lde_end(&self) -> bool {
        (self.ord & LAST_LONG_ENTRY) == LAST_LONG_ENTRY
    }
    pub fn as_bytes_array(&self) -> [u8; 32] {
        unsafe { core::ptr::read_unaligned(self as *const Self as *const [u8; 32]) }
    }
    pub fn as_bytes_array_mut(&mut self) -> &mut [u8; 32] {
        unsafe { &mut *(self as *mut Self as *mut [u8; 32]) }
    }
    pub fn to_bytes_array(&self) -> [u8; 32] {
        let mut buf = [0; 32];
        buf.copy_from_slice(self.as_bytes());
        buf
    }
}
