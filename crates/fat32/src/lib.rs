#![no_std] // if test in std environment, comment this line
pub mod bpb;
pub mod cache;
pub mod device;
pub mod dir;
pub mod entry;
pub mod fat;
pub mod fs;
pub mod vf;

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::iter::Iterator;
use core::str;
extern crate alloc;
#[cfg(feature = "time-tracer")]
#[macro_use]
extern crate time_tracer;

pub use bpb::*;
pub use cache::*;
pub use device::*;
pub use dir::*;
pub use entry::*;
pub use fat::*;
pub use fs::*;
pub use vf::*;

// Cluster
pub const FREE_CLUSTER: u32 = 0x00000000;
pub const BAD_CLUSTER: u32 = 0x0FFF_FFF7;
/// EOC: End of Cluster Chain
/// Microsoft operating system FAT drivers use the EOC value 0x0FFF for FAT12, 0xFFFF for FAT16,
/// and 0x0FFFFFFF for FAT32 when they set the contents of a cluster to the EOC mark.
// pub const END_OF_CLUSTER: u32 = 0x0FFFFFFF; linux mkfs fat32 再 mount 后发现 EOC 的值为 0x0FFFFFF8
pub const END_OF_CLUSTER: u32 = 0x0FFF_FFF8;
pub const CLUSTER_MASK: u32 = 0x0FFF_FFFF;
pub const NEW_VIRT_FILE_CLUSTER: u32 = 0;

// mark as root directory entry cluster number (root directory entry is not actually saved on the disk)
pub const ROOT_DIR_ENTRY_CLUSTER: u32 = 0;

// File Attribute
pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_ARCHIVE: u8 = 0x20;
pub const ATTR_LONG_NAME: u8 = ATTR_READ_ONLY | ATTR_HIDDEN | ATTR_SYSTEM | ATTR_VOLUME_ID;

// Directory Entry
pub const DIRENT_SIZE: usize = 32;

// Cache Limit
pub const BLOCK_CACHE_LIMIT: usize = 64;

// Charactor
pub const SPACE: u8 = 0x20;
pub const DOT: u8 = 0x2E;
pub const ROOT: u8 = 0x2F;

// Only used for test in std environment
pub const BLOCK_NUM: u32 = 0x4000;
pub const ROOT_DIR_CLUSTER: u32 = 2;

/// BPB Bytes Per Sector
pub const BLOCK_SIZE: usize = 512;
pub const CACHE_SIZE: usize = 512;
pub const FAT_BUFFER_SIZE: usize = 512;
pub const DIR_BUFFER_SIZE: usize = 512;
pub const FILE_BUFFER_SIZE: usize = 512;

// Directory Entry Name Length Capicity
pub const LONG_NAME_LEN_CAP: usize = 13;
pub const SHORT_NAME_LEN_CAP: usize = 11;

/// For Short Directory Entry Name[0] and Long Directory Entry Ord
///
/// Deleted
pub const DIR_ENTRY_UNUSED: u8 = 0xE5;
/// For Short Directory Entry Name[0]
pub const DIR_ENTRY_LAST_AND_UNUSED: u8 = 0x00;
/// For Long Directory Entry Ord as the last entry mask
pub const LAST_LONG_ENTRY: u8 = 0x40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDeviceError {
    ClusterChain(ClusterChainErr),
    Dir(DirError),
}

pub(crate) fn read_le_u32(input: &[u8]) -> u32 {
    let (int_bytes, _) = input.split_at(core::mem::size_of::<u32>());
    u32::from_le_bytes(int_bytes.try_into().unwrap())
}

/// Split long file name and return a string array
pub fn long_name_split(name: &str) -> Vec<[u16; 13]> {
    let mut name: Vec<u16> = name.encode_utf16().collect();
    let len = name.len(); // note that name &str end with '\0'

    // count how many long directory entries are needed
    let lfn_cnt = (len + LONG_NAME_LEN_CAP - 1) / LONG_NAME_LEN_CAP;
    if len < lfn_cnt * LONG_NAME_LEN_CAP {
        name.push(0x0000);
        while name.len() < (lfn_cnt * LONG_NAME_LEN_CAP) as usize {
            name.push(0xFFFF);
        }
    }
    name.chunks(LONG_NAME_LEN_CAP as usize)
        .map(|x| {
            let mut arr = [0u16; 13];
            arr.copy_from_slice(x);
            arr
        })
        .collect()
}

/// Split file name and extension
pub fn split_name_ext(name: &str) -> (&str, &str) {
    match name {
        "." => return (".", ""),
        ".." => return ("..", ""),
        _ => {
            let mut name_and_ext: Vec<&str> = name.split(".").collect();
            if name_and_ext.len() == 1 {
                // if no ext, push a empty string
                name_and_ext.push("");
            }
            (name_and_ext[0], name_and_ext[1])
        }
    }
}

/// Format short file name to directory entry
pub fn short_name_format(name: &str) -> ([u8; 8], [u8; 3]) {
    let (name, ext) = split_name_ext(name);
    let name_bytes = name.as_bytes();
    let ext_bytes = ext.as_bytes();
    let mut f_name = [0u8; 8];
    let mut f_ext = [0u8; 3];
    for i in 0..8 {
        if i >= name_bytes.len() {
            f_name[i] = 0x20; // fullfill with 0x20 (ascci space) if not enough
        } else {
            f_name[i] = (name_bytes[i] as char).to_ascii_uppercase() as u8;
        }
    }
    for i in 0..3 {
        if i >= ext_bytes.len() {
            f_ext[i] = 0x20; // fullfill with 0x20 (ascci space) if not enough
        } else {
            f_ext[i] = (ext_bytes[i] as char).to_ascii_uppercase() as u8;
        }
    }
    (f_name, f_ext)
}

// Generate short file name from long file name
pub fn generate_short_name(long_name: &str) -> String {
    let (name_, ext_) = split_name_ext(long_name);
    let name = name_.as_bytes();
    let extension = ext_.as_bytes();
    let mut short_name = String::new();
    // take the first 6 characters of the long file name and add "~1" to form a short file name,
    // and duplicate names are not currently supported.
    for i in 0..6.min(name.len()) {
        short_name.push((name[i] as char).to_ascii_uppercase())
    }
    short_name.push('~');
    short_name.push('1');
    while short_name.len() < 8 {
        // fullfill with 0x20 (ascci space) if not enough
        short_name.push(0x20 as char);
    }
    let ext_len = extension.len();
    for i in 0..3 {
        if i >= ext_len {
            // fullfill with 0x20 (ascci space) if not enough
            short_name.push(0x20 as char);
        } else {
            short_name.push((extension[i] as char).to_ascii_uppercase());
        }
    }
    short_name
}

// Following May Unused

// Signature
pub const LEAD_SIGNATURE: u32 = 0x41615252;
pub const STRUCT_SIGNATURE: u32 = 0x61417272;
pub const TRAIL_SIGNATURE: u32 = 0xAA550000;

// Cluster
pub const MAX_CLUSTER_FAT12: usize = 4085;
pub const MAX_CLUSTER_FAT16: usize = 65525;
pub const MAX_CLUSTER_FAT32: usize = 268435445;

// Name Status for Short Directory Entry
pub const ALL_UPPER_CASE: u8 = 0x00;
pub const ALL_LOWER_CASE: u8 = 0x08;
pub const ORIGINAL: u8 = 0x0F;

/// The two reserved clusters at the start of the FAT, and FAT[1] high bit mask as follows:
/// Bit ClnShutBitMask -- If bit is 1, volume is "clean". If bit is 0, volume is "dirty".
/// Bit HrdErrBitMask  -- If this bit is 1, no disk read/write errors were encountered.
///                       If this bit is 0, the file system driver encountered a disk I/O error on the Volume
///                       the last time it was mounted, which is an indicator that some sectors may have gone bad on the volume.
pub const CLN_SHUT_BIT_MASK_FAT32: u32 = 0x08000000;
pub const HRD_ERR_BIT_MASK_FAT32: u32 = 0x04000000;

// Q: The default maximum number of lde does not exceed 0x40?
//    But the maximum number of files within a directory of a FAT
//    file system is 65,536. So, how to deal with lfn.ord?
//
// A: DO NOT misunderstand the meaning of this mask.
//    This mask should be for ord in the same file. The long
//    file name of a long directory entry only has 13 unicode
//    characters. When the file name exceeds 13 characters,
//    multiple long directory entries are required.
// pub const LAST_LONG_ENTRY: u8 = 0x40;

#[allow(unused)]
pub(crate) fn is_illegal(chs: &str) -> bool {
    let illegal_char = "\\/:*?\"<>|";
    for ch in illegal_char.chars() {
        if chs.contains(ch) {
            return true;
        }
    }
    false
}
#[allow(unused)]
pub(crate) fn read_le_u16(input: &[u8]) -> u16 {
    let (int_bytes, _) = input.split_at(core::mem::size_of::<u16>());
    u16::from_le_bytes(int_bytes.try_into().unwrap())
}
