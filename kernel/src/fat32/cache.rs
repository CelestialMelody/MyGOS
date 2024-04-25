//! Block cache
//! The reason why BlockCache uses Vec<u8>: https://github.com/rcore-os/rCore-Tutorial-v3/pull/79
//! alse see atricle: https://gitlab.eduxiji.net/2019301887/oskernel2022-npucore/-/blob/master/Doc/debug/Virt-IO%E9%A9%B1%E5%8A%A8bug.md

use super::device::BlockDevice;
use super::{BLOCK_CACHE_LIMIT, BLOCK_SIZE};

// use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::lazy::Lazy;
// use core::num::NonZeroUsize;
use core::ops::Drop;
use core::ops::FnOnce;
// use lazy_static::*;
use lru::LruCache;
use spin::{Mutex, RwLock};

pub trait Cache {
    /// The read-only mapper to the block cache
    ///
    /// - `offset`: offset in cache
    /// - `f`: a closure to read
    fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V;
    /// The mutable mapper to the block cache
    ///
    /// - `offset`: offset in cache
    /// - `f`: a closure to write
    fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V;
    /// Tell cache to write back
    ///
    /// - `block_ids`: block ids in this cache
    /// - `block_device`: The pointer to the block_device.
    fn sync(&mut self);
}

pub struct BlockCache {
    // cache: [u8; BLOCK_SIZE],
    cache: Vec<u8>,
    // the block id in the disk not in the cluster
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
    modified: bool,
}

impl BlockCache {
    // load a block from the disk
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut cache = vec![0 as u8; BLOCK_SIZE];
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    }
    fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SIZE);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }
    fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SIZE);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }
}

impl Cache for BlockCache {
    fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }
    fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }
    // write the content back to disk
    fn sync(&mut self) {
        // TODO need to consider reference count?
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache)
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}

pub struct BlockCacheManager {
    lru: LruCache<usize, Arc<RwLock<BlockCache>>>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            /// Creates a new LRU Cache that never automatically evicts items.
            lru: LruCache::unbounded(),
        }
    }
    // get a block cache by block id
    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<RwLock<BlockCache>> {
        // if the block is already in lru_cache, just return the copy
        if let Some(pair) = self.lru.get(&block_id) {
            Arc::clone(pair)
        } else {
            // if the block is not in lru_cache, create a new block_cache
            let block_cache = Arc::new(RwLock::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            // if the lru_cache is full, write back the least recently used block_cache
            if self.lru.len() == BLOCK_CACHE_LIMIT {
                let (_, peek_cache) = self.lru.peek_lru().unwrap();
                if Arc::strong_count(peek_cache) == 1 {
                    self.lru.pop_lru();
                    self.lru.put(block_id, Arc::clone(&block_cache));
                }
            } else {
                self.lru.put(block_id, Arc::clone(&block_cache));
            }
            block_cache
        }
    }
    pub fn sync_all(&mut self) {
        for (_, block_cache) in self.lru.iter() {
            block_cache.write().sync();
        }
    }
}

// create a block cache manager with 64 blocks
// lazy_static! {
//     pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
//         Mutex::new(BlockCacheManager::new());
// }

// TODO
pub static BLOCK_CACHE_MANAGER: Lazy<Mutex<BlockCacheManager>> =
    Lazy::new(|| Mutex::new(BlockCacheManager::new()));

// used for external modules
pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<RwLock<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}
pub fn sync_all() {
    BLOCK_CACHE_MANAGER.lock().sync_all();
}
