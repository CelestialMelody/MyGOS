use alloc::sync::{Arc, Weak};
// use fat32::{VirtFile, BLOCK_SIZE};
use crate::fat32::{VirtFile, BLOCK_SIZE};
use spin::Mutex;

use crate::{
    consts::PAGE_SIZE,
    mm::{alloc_frame, FrameTracker, MapPermission},
    syscall::impls::Errno,
};

// The difference compared to MmapPage is that it does not require a virtual address.
pub struct FilePage {
    /// Immutable page permission
    pub permission: MapPermission,
    /// Physical data frame
    pub data_frame: FrameTracker,
    pub file_info: Option<Mutex<FilePageInfo>>,
}

#[derive(PartialEq, Clone, Copy)]
pub enum DataState {
    Dirty,
    Load,
    Unload,
}
pub struct FilePageInfo {
    /// Start offset of this page at its related file
    pub file_offset: usize,
    /// Data block state
    pub data_states: [DataState; PAGE_SIZE / BLOCK_SIZE],
    /// Inode that this page related to
    inode: Weak<VirtFile>,
}
#[allow(unused)]
impl FilePage {
    pub fn new(perm: MapPermission, offset: usize, inode: Arc<VirtFile>) -> Self {
        let data_frame = alloc_frame().expect("failed to alloc frame");
        assert!(offset % PAGE_SIZE == 0);
        let file_info = FilePageInfo {
            file_offset: offset,
            data_states: [DataState::Unload; PAGE_SIZE / BLOCK_SIZE],
            inode: Arc::downgrade(&inode),
        };
        Self {
            permission: perm,
            data_frame,
            file_info: Some(Mutex::new(file_info)),
        }
    }
    pub fn as_mut<T>(&self) -> &'static mut T {
        self.data_frame.ppn.as_mut()
    }

    pub fn as_ref<T>(&self) -> &'static T {
        self.data_frame.ppn.as_ref()
    }

    /// Read this page.
    /// `offset`: page offset
    pub fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Errno> {
        if offset >= PAGE_SIZE {
            Err(Errno::E2BIG)
        } else {
            let mut end = offset + buf.len();
            if end > PAGE_SIZE {
                end = PAGE_SIZE;
            }
            self.load_buffer_if_needed(offset, end)?;
            buf.copy_from_slice(&&self.data_frame.ppn.as_bytes_array()[offset..end]);
            Ok(end - offset)
        }
    }
    /// Write this page.
    /// `offset`: page offset
    pub fn write(&self, offset: usize, buf: &[u8]) -> Result<usize, Errno> {
        // trace!("[Page::write]: page addr {:#x}", self as *const Self as usize);
        trace!(
            "[Page::write]: page addr {:#x}",
            self as *const Self as usize
        );
        if offset >= PAGE_SIZE {
            Err(Errno::E2BIG)
        } else {
            let mut end = offset + buf.len();
            if end > PAGE_SIZE {
                end = PAGE_SIZE;
            }
            self.mark_buffer_dirty_if_needed(offset, end)?;
            self.data_frame.ppn.as_bytes_array()[offset..end].copy_from_slice(buf);
            Ok(end - offset)
        }
    }

    /// Sync all buffers if needed
    pub fn sync(&self) -> Result<(), Errno> {
        let file_info = self.file_info.as_ref().unwrap().lock();
        let inode = file_info.inode.upgrade().ok_or(Errno::EBADF)?;
        // let file_size = inode.file_size();
        // log::trace!("[Page::sync] sync page, file offset {:#x}",file_info.file_offset);
        log::trace!(
            "[Page::sync] sync page, file offset {:#x}",
            file_info.file_offset
        );
        for idx in 0..PAGE_SIZE / BLOCK_SIZE {
            match file_info.data_states[idx] {
                DataState::Dirty => {
                    let page_offset = idx * BLOCK_SIZE;
                    let file_offset = file_info.file_offset + page_offset;
                    // log::trace!("[Page::sync] sync block of the page, file offset {:#x}",file_offset);
                    log::trace!(
                        "[Page::sync] sync block of the page, file offset {:#x}",
                        file_offset
                    );
                    // // In case of truncate
                    // if file_size <= file_offset {
                    //     // info!("[Page::sync] file has been truncated, now len {:#x}, page's file offset {:#x}", file_size, file_offset);
                    //     return Ok(());
                    // }
                    let data = &self.data_frame.ppn.as_bytes_array()
                        [page_offset..page_offset + BLOCK_SIZE]
                        .to_vec();

                    // inode.write_from_direct(file_offset, data);
                    inode.write_at(file_offset, &data);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Load all buffers
    pub fn load_all_buffers(&self) -> Result<(), Errno> {
        // trace!("[Page::load_all_buffers]: page addr {:#x}", self.bytes_array_ptr() as usize);
        trace!(
            "[Page::load_all_buffers]: page addr {:#x}",
            self.as_bytes_array_ptr() as usize
        );
        let len = PAGE_SIZE;
        self.load_buffer_if_needed(0, len);
        Ok(())
    }

    /// Get the raw pointer of this page
    pub fn as_bytes_array_ptr(&self) -> *const u8 {
        self.data_frame.ppn.as_bytes_array().as_ptr()
    }

    /// Get the bytes array of this page
    pub fn as_bytes_array(&self) -> &'static [u8] {
        self.data_frame.ppn.as_bytes_array()
    }

    fn load_buffer_if_needed(&self, start_off: usize, end_off: usize) -> Result<(), Errno> {
        let start_buffer_idx = start_off / BLOCK_SIZE;
        let end_buffer_idx = (end_off - 1 + BLOCK_SIZE) / BLOCK_SIZE;

        let mut file_info = self.file_info.as_ref().unwrap().lock();
        for idx in start_buffer_idx..end_buffer_idx {
            if file_info.data_states[idx] == DataState::Unload {
                // trace!("outdated block, idx {}, start_page_off {:#x}",idx,start_off);
                trace!(
                    "outdated block, idx {}, start_page_off {:#x}",
                    idx,
                    start_off
                );
                let page_offset = idx * BLOCK_SIZE;
                let file_offset = page_offset + file_info.file_offset;
                let dst = &mut self.data_frame.ppn.as_bytes_array()
                    [page_offset..page_offset + BLOCK_SIZE];

                // let src = file_info
                //     .inode
                //     .upgrade()
                //     .unwrap()
                //     .read_at_direct(file_offset, BLOCK_SIZE);
                let mut src = vec![0u8; BLOCK_SIZE];
                file_info
                    .inode
                    .upgrade()
                    .unwrap()
                    .read_at(file_offset, &mut src);
                dst.copy_from_slice(&src);
                file_info.data_states[idx] = DataState::Load;
            }
        }
        Ok(())
    }

    fn mark_buffer_dirty_if_needed(&self, start_off: usize, end_off: usize) -> Result<(), Errno> {
        let start_buffer_idx = start_off / BLOCK_SIZE;
        let end_buffer_idx = (end_off - 1 + BLOCK_SIZE) / BLOCK_SIZE;
        trace!("start {}, end {}", start_buffer_idx, end_buffer_idx);

        let mut file_info = self.file_info.as_ref().unwrap().lock();

        for idx in start_buffer_idx..end_buffer_idx {
            if file_info.data_states[idx] == DataState::Unload {
                let page_offset = idx * BLOCK_SIZE;
                let file_offset = page_offset + file_info.file_offset;
                let dst = &mut self.data_frame.ppn.as_bytes_array()
                    [page_offset..page_offset + BLOCK_SIZE];
                // let src = file_info
                //     .inode
                //     .upgrade()
                //     .unwrap()
                //     .read_at_direct(file_offset, BLOCK_SIZE);

                let mut src = vec![0u8; BLOCK_SIZE];
                file_info
                    .inode
                    .upgrade()
                    .unwrap()
                    .read_at(file_offset, &mut src);
                dst.copy_from_slice(src.as_slice());
                // trace!("outdated block, idx {}, start_page_off {:#x}",idx,start_off);
                trace!(
                    "outdated block, idx {}, start_page_off {:#x}",
                    idx,
                    start_off
                );
                file_info.data_states[idx] = DataState::Load;
            }
            if file_info.data_states[idx] != DataState::Dirty {
                file_info.data_states[idx] = DataState::Dirty;
            }
        }
        Ok(())
    }
}
