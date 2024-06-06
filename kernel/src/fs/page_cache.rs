//! Before using the page cache, file reads and writes were directly interacting
//! with FAT32. Now, there is an additional layer of caching between the kernel and FAT32.

use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
    vec::Vec,
};

use fat32::VirtFile;
// use crate::fat32::VirtFile;

use spin::RwLock;

use crate::{consts::PAGE_SIZE, mm::MapPermission, syscall::impls::Errno};

use super::FilePage;

pub struct PageCache {
    inode: Option<Weak<VirtFile>>,
    // page number -> page
    pub pages: RwLock<BTreeMap<usize, Arc<FilePage>>>,
}

#[allow(unused)]
impl PageCache {
    pub fn new(inode: Arc<VirtFile>) -> Self {
        Self {
            inode: Some(Arc::downgrade(&inode)),
            pages: RwLock::new(BTreeMap::new()),
        }
    }
    fn lookup(&self, offset: usize) -> Option<Arc<FilePage>> {
        self.pages.read().get(&(offset / PAGE_SIZE)).cloned()
    }
    pub fn insert(&self, offset: usize, page: FilePage) {
        debug_assert!(self
            .pages
            .write()
            .insert(offset / PAGE_SIZE, Arc::new(page))
            .is_none())
    }
    /// Get a page according to the given file offset
    pub fn get_page(
        &self,
        offset: usize,
        map_perm: Option<MapPermission>,
    ) -> Result<Arc<FilePage>, Errno> {
        // trace!("[PageCache]: get page at file offset {:#x}", offset);
        trace!("[PageCache]: get page at file offset {:#x}", offset);
        let page_start_offset = offset & !(PAGE_SIZE - 1);
        if let Some(page) = self.lookup(offset) {
            Ok(page)
        } else {
            let page = Arc::new(FilePage::new(
                map_perm.unwrap_or(MapPermission::R | MapPermission::W),
                page_start_offset,
                self.inode.as_ref().unwrap().upgrade().unwrap(),
            ));
            self.pages.write().insert(offset / PAGE_SIZE, page.clone());
            Ok(page)
        }
    }
    /// Flush all pages to disk if needed
    pub fn sync(&self) -> Result<(), Errno> {
        let mut page_set: Vec<Arc<FilePage>> = Vec::new();
        for (_, page) in self.pages.read().iter() {
            page_set.push(page.clone());
        }
        for page in page_set {
            page.sync()?;
        }
        Ok(())
    }

    pub fn remove(&self, offset: usize) {
        self.pages.write().remove(&(offset / PAGE_SIZE));
    }
}
