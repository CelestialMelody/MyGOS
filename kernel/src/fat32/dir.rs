//! Directory file traits definition and implementation for VirtFile
//!
// The layout of directory entries in a directory file on disk
// (from low address to high address) is as follows:
// fileA_lde_n
// fileA_lde_n-1
// ...
// fileA_lde_1
// fileA_sde
// fileB_lde_n
// fileB_lde_n-1
// ...
// fileB_lde_1
// fileB_sde
// ...
// Note: Fat32 specifies that the size of the directory file is 0

use super::entry::{LongDirEntry, ShortDirEntry};
use super::vf::{DirEntryPos, VirtFile, VirtFileType};

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::clone::Clone;
use core::convert::From;
use core::option::Option::{self, None, Some};
use core::result::Result::{self, Err, Ok};
use core::{assert, assert_eq};
use spin::RwLock;

use super::{generate_short_name, long_name_split, short_name_format, split_name_ext};
use super::{
    ALL_UPPER_CASE, ATTR_DIRECTORY, ATTR_LONG_NAME, DIRENT_SIZE, DIR_ENTRY_UNUSED, LAST_LONG_ENTRY,
    NEW_VIRT_FILE_CLUSTER,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirError {
    NoMatchDir,
    NoMatchFile,
    NoMatch,
    IllegalChar,
    DirHasExist,
    FileHasExist,
    NotDir,
    ListLFNIllegal,
    CreateFileError,
    MissingName,
}

pub trait Dir {
    fn find(&self, path: Vec<&str>) -> Result<Arc<VirtFile>, DirError>;
    fn create(&self, name: &str, file_type: VirtFileType) -> Result<VirtFile, DirError>;
    fn ls(&self) -> Result<Vec<String>, DirError>;
    fn remove(&self, path: Vec<&str>) -> Result<(), DirError>;
}

impl Dir for VirtFile {
    fn find(&self, path: Vec<&str>) -> Result<Arc<VirtFile>, DirError> {
        #[cfg(feature = "time-tracer")]
        time_trace!("find");
        let len = path.len();
        if len == 0 {
            return Ok(Arc::new(self.clone()));
        }
        let mut current = self.clone();
        for i in 0..len {
            if path[i] == "" || path[i] == "." {
                continue;
            }
            if !current.is_dir() {
                return Err(DirError::NotDir);
            }
            if let Some(vfile) = current.find_by_name(path[i]) {
                current = vfile;
            } else {
                return Err(DirError::NoMatch);
            }
        }
        Ok(Arc::new(current))
    }
    fn remove(&self, path: Vec<&str>) -> Result<(), DirError> {
        match self.find(path) {
            Ok(file) => {
                file.clear();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    fn ls(&self) -> Result<Vec<String>, DirError> {
        match self.ls_with_attr() {
            Ok(v) => {
                let mut name = Vec::new();
                for i in v {
                    name.push(i.0);
                }
                Ok(name)
            }
            Err(e) => Err(e),
        }
    }

    // Dir Functions

    fn create(&self, name: &str, file_type: VirtFileType) -> Result<VirtFile, DirError> {
        // serach same name file
        assert!(self.is_dir());
        let option = self.find_by_name(name);
        if let Some(file) = option {
            if file.virt_file_type() == file_type {
                return Err(DirError::FileHasExist);
            }
        }
        let (name_, ext_) = split_name_ext(name);
        // serach empty position for dirent
        let mut entry_offset: usize;

        match self.empty_entry_index() {
            Ok(offset) => {
                entry_offset = offset;
            }
            Err(e) => {
                return Err(e);
            }
        }
        // low -> high
        // lfn(n) -> lfn(n-1) -> .. -> lfn(1) -> sfn
        let mut sde: ShortDirEntry;
        if name_.len() > 8 || ext_.len() > 3 {
            // generate short name and dirent from long name
            let short_name = generate_short_name(name);
            let (_name, _ext) = short_name_format(short_name.as_str());
            sde = ShortDirEntry::new(NEW_VIRT_FILE_CLUSTER, &_name, &_ext, file_type);
            sde.set_name_case(ALL_UPPER_CASE); // 可能不必要

            // Split long file name
            let mut lfn_vec = long_name_split(name);
            // Number of long file name directory entries to be created
            let lfn_cnt = lfn_vec.len();
            // Write long name directory entries one by one
            for i in 0..lfn_cnt {
                // Fill in the long file name directory entry in reverse order to avoid name confusion
                let mut order: u8 = (lfn_cnt - i) as u8;
                if i == 0 {
                    // The last long file name directory entry, perform bitwise OR with 0x40 and write the result
                    order |= 0x40;
                }
                // Initialize the long file name directory entry
                let lde = LongDirEntry::new_form_name_slice(
                    order,
                    lfn_vec.pop().unwrap(),
                    sde.gen_check_sum(),
                );
                // Write the long file name directory entry
                let write_size = self.write_at(entry_offset, lde.as_bytes());
                assert_eq!(write_size, DIRENT_SIZE);
                // Update the write position
                entry_offset += DIRENT_SIZE;
            }
        } else {
            // short name
            let (_name, _ext) = short_name_format(name);
            sde = ShortDirEntry::new(NEW_VIRT_FILE_CLUSTER, &_name, &_ext, file_type);
            sde.set_name_case(ALL_UPPER_CASE); // maybe not necessary

            // Linux will create a long file name directory entry when
            // creating a file to handle the case of the file
            let order: u8 = 1 | 0x40;
            let name_array = long_name_split(name)[0];
            let lde = LongDirEntry::new_form_name_slice(order, name_array, sde.gen_check_sum());
            let write_size = self.write_at(entry_offset, lde.as_bytes());
            assert_eq!(write_size, DIRENT_SIZE);
            entry_offset += DIRENT_SIZE;
        }

        // write short dirent(there is also a short dirent for long file name)
        let wirte_size = self.write_at(entry_offset, sde.as_bytes());
        assert_eq!(wirte_size, DIRENT_SIZE);
        assert!(
            self.first_cluster() >= 2,
            "[fat32::Dir::create] first_cluster:{}",
            self.first_cluster()
        );

        // validation
        if let Some(file) = self.find_by_name(name) {
            // 如果是目录类型, 需要创建.和..
            if file_type == VirtFileType::Dir {
                // First, write ".." to make the directory obtain the first cluster
                // (otherwise, increase_size will not allocate a cluster and will
                // return directly, causing the first_cluster to be 0, leading to a panic)
                let (_name, _ext) = short_name_format("..");
                let mut parent_sde = ShortDirEntry::new(
                    self.first_cluster() as u32,
                    &_name,
                    &_ext,
                    VirtFileType::Dir,
                );
                // According to FAT32 specifications, the directory file size is 0,
                // so do not update the size of the directory file.
                file.write_at(DIRENT_SIZE, parent_sde.as_bytes_mut());

                let (_name, _ext) = short_name_format(".");
                let mut self_sde = ShortDirEntry::new(
                    file.first_cluster() as u32,
                    &_name,
                    &_ext,
                    VirtFileType::Dir,
                );
                file.write_at(0, self_sde.as_bytes_mut());
            }
            Ok(file)
        } else {
            Err(DirError::CreateFileError)
        }
    }
}

impl VirtFile {
    // Dir Functions
    fn find_by_lfn(&self, name: &str) -> Option<VirtFile> {
        let name_vec = long_name_split(name);
        let name_cnt = name_vec.len();

        let mut index = 0;
        let mut lde = LongDirEntry::empty();
        let mut lde_pos_vec: Vec<DirEntryPos> = Vec::new();
        let name_last = name_vec[name_cnt - 1].clone();
        loop {
            let mut read_size = self.read_at(index, lde.as_bytes_mut());
            if read_size != DIRENT_SIZE {
                return None;
            }

            // First, match the last long file name directory entry,
            // which corresponds to the last block of the long file name.
            if lde.attr() == ATTR_LONG_NAME // must be long name
            && lde.name_utf16() == name_last
            {
                let mut order = lde.order();
                if order & LAST_LONG_ENTRY == 0 || order == DIR_ENTRY_UNUSED {
                    index += DIRENT_SIZE;
                    continue;
                }
                // Restore the correct order value for 'order'
                order = order ^ LAST_LONG_ENTRY;
                // If the number of long file name directory entries does not match,
                // skip and continue searching
                if order as usize != name_cnt {
                    index += DIRENT_SIZE;
                    continue;
                }
                // If the order matches, enter a loop to continue matching the
                // long name directory entries
                let mut is_match = true;
                for i in 1..order as usize {
                    read_size = self.read_at(index + i * DIRENT_SIZE, lde.as_bytes_mut());
                    if read_size != DIRENT_SIZE {
                        return None;
                    }
                    // Match the previous name field, and exit if it fails
                    if lde.name_utf16() != name_vec[name_cnt - 1 - i]
                        || lde.attr() != ATTR_LONG_NAME
                    {
                        is_match = false;
                        break;
                    }
                }
                if is_match {
                    // If successful, read the short directory entry for verification
                    let checksum = lde.check_sum();
                    let mut sde = ShortDirEntry::empty();
                    let sde_offset = index + name_cnt * DIRENT_SIZE;
                    read_size = self.read_at(sde_offset, sde.as_bytes_mut());
                    if read_size != DIRENT_SIZE {
                        return None;
                    }
                    if !sde.is_deleted() && checksum == sde.gen_check_sum() {
                        let sde_pos = self.dirent_cluster_pos(sde_offset).unwrap();
                        for i in 0..order as usize {
                            // The positions of the long name directory entries are stored,
                            // with the first one at the top of the stack.
                            let lde_pos = self.dirent_cluster_pos(index + i * DIRENT_SIZE);
                            lde_pos_vec.push(lde_pos.unwrap());
                        }
                        let file_type = if sde.attr() == ATTR_DIRECTORY {
                            VirtFileType::Dir
                        } else {
                            VirtFileType::File
                        };

                        let clus_chain = self.generate_cluster_chain(sde_offset);

                        return Some(VirtFile::new(
                            String::from(name),
                            sde_pos,
                            lde_pos_vec,
                            Arc::clone(&self.fs),
                            Arc::clone(&self.device),
                            Arc::new(RwLock::new(clus_chain)),
                            file_type,
                        ));
                    }
                }
            }
            index += DIRENT_SIZE;
        }
    }

    fn find_by_sfn(&self, name: &str) -> Option<VirtFile> {
        let name = name.to_ascii_uppercase();
        let mut sde = ShortDirEntry::empty();
        let mut index = 0;
        loop {
            let read_size = self.read_at(index, sde.as_bytes_mut());
            if read_size != DIRENT_SIZE {
                return None;
            }

            // check if the names are the same:
            if !sde.is_deleted() && name == sde.get_name_uppercase() {
                let sde_pos = self.dirent_cluster_pos(index).unwrap();
                let lde_pos_vec: Vec<DirEntryPos> = Vec::new();
                let file_type = if sde.attr() == ATTR_DIRECTORY {
                    VirtFileType::Dir
                } else {
                    VirtFileType::File
                };
                let clus_chain = self.generate_cluster_chain(index);
                return Some(VirtFile::new(
                    String::from(name),
                    sde_pos,
                    lde_pos_vec,
                    Arc::clone(&self.fs),
                    Arc::clone(&self.device),
                    Arc::new(RwLock::new(clus_chain)),
                    file_type,
                ));
            } else {
                index += DIRENT_SIZE;
                continue;
            }
        }
    }

    pub fn find_by_name(&self, name: &str) -> Option<VirtFile> {
        assert!(self.is_dir());
        let (name_, ext_) = split_name_ext(name);
        if name_.len() > 8 || ext_.len() > 3 {
            return self.find_by_lfn(name);
        } else {
            return self.find_by_sfn(name);
        }
    }

    // Find an available directory entry and return the offset.
    // If there are not enough clusters, it will also return the corresponding offset.
    fn empty_entry_index(&self) -> Result<usize, DirError> {
        if !self.is_dir() {
            return Err(DirError::NotDir);
        }
        let mut sde = ShortDirEntry::empty();
        let mut index = 0;
        loop {
            let read_size = self.read_at(index, sde.as_bytes_mut());
            // Reached the end of the directory file ->
            // exceeded dir_size, need to allocate a new cluster ->
            // handled in write_at ->
            // increase_size
            if read_size == 0 || sde.is_empty() {
                return Ok(index);
            } else {
                index += DIRENT_SIZE;
            }
        }
    }

    pub fn virt_file_type(&self) -> VirtFileType {
        if self.is_dir() {
            VirtFileType::Dir
        } else {
            VirtFileType::File
        }
    }

    // Return a tuple where the first element is the file name and the
    // second element is the file attribute (file or directory).
    pub fn ls_with_attr(&self) -> Result<Vec<(String, u8)>, DirError> {
        if !self.is_dir() {
            return Err(DirError::NotDir);
        }
        let mut list: Vec<(String, u8)> = Vec::new();
        let mut entry = LongDirEntry::empty();
        let mut offset = 0usize;
        loop {
            let read_size = self.read_at(offset, entry.as_bytes_mut());
            // Finished reading
            if read_size != DIRENT_SIZE || entry.is_empty() {
                return Ok(list);
            }
            // Skip if the file is marked as deleted
            if entry.is_deleted() {
                offset += DIRENT_SIZE;
                continue;
            }
            if entry.attr() != ATTR_LONG_NAME {
                // Short file name
                let sde: ShortDirEntry = unsafe { core::mem::transmute(entry) };
                list.push((sde.get_name_lowercase(), sde.attr()));
            } else {
                // Long file name
                // If it's a long file name directory entry, it must be the last
                // segment of the long file name
                let mut name = String::new();
                let order = entry.order() ^ LAST_LONG_ENTRY;
                for _ in 0..order {
                    name.insert_str(0, &entry.name().as_str());
                    offset += DIRENT_SIZE;
                    let read_size = self.read_at(offset, entry.as_bytes_mut());
                    if read_size != DIRENT_SIZE || entry.is_empty() {
                        return Err(DirError::ListLFNIllegal);
                    }
                }
                list.push((name.clone(), entry.attr()));
            }
            offset += DIRENT_SIZE;
        }
    }
}
