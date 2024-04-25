//! **当前代码实现规定**
//! - 从数据区开始, 对 cluster 进行编号, 编号从 2 开始;
//!   计算在磁盘中的偏移 offset = BLOCK_SIZE * (bpb.first_data_sector + (cluster - 2) * bpb.sector_per_cluster)
//! - block_id 在存储介质从 0 开始 从 0 编号;
//!   计算在磁盘中的偏移 offset = BLOCK_SIZE * block_id
//! - 其他命名尽量容易理解 如 block_id_in_cluster 为簇内块号

use super::cache::get_block_cache;
use super::cache::Cache;
use super::device::BlockDevice;
use super::read_le_u32;
use super::{BLOCK_SIZE, CLUSTER_MASK, END_OF_CLUSTER};

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::assert;
use core::clone::Clone;
use core::fmt::Debug;
use core::iter::Iterator;
use core::option::Option;
use core::option::Option::{None, Some};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterChainErr {
    ReadError,
    WriteError,
    NonePreviousCluster,
    NoneNextCluster,
}

#[derive(Clone)]
/// Cluster Chain in FAT Table.
///
/// Like a Dual-Linked List.
//
//  单个文件/目录的簇号链表
//  注意, 整个 Fat 表的簇号从 2 开始, 0 和 1 为保留簇号;
//  根据 cluster_id 求出偏移时, 数据区以 cluster_size 为单位从 0 开始计算, cluster_id - 2
pub struct ClusterChain {
    pub(crate) device: Arc<dyn BlockDevice>,
    // FAT表的偏移, 也是 start_cluster 的第一个 sector 的偏移
    // 目前仅指 FAT1
    // 可以通过 BIOSParameterBlock::fat1() 方法获取
    pub(crate) fat1_offset: usize,
    // 簇号表
    pub(crate) cluster_vec: Vec<u32>,
}

#[allow(unused)]
impl ClusterChain {
    pub(crate) fn new(cluster: u32, device: Arc<dyn BlockDevice>, fat_offset: usize) -> Self {
        if (cluster >= 2) {
            Self {
                device: Arc::clone(&device),
                fat1_offset: fat_offset,
                cluster_vec: vec![cluster],
            }
        } else {
            Self {
                device: Arc::clone(&device),
                fat1_offset: fat_offset,
                cluster_vec: vec![],
            }
        }
    }
    pub(crate) fn refresh(&mut self, start_cluster: u32) {
        self.cluster_vec = vec![start_cluster];
    }
    /// Initail cluster_vec from disk.
    ///
    // 从磁盘读取簇号链表, 减少磁盘读取次数
    pub(crate) fn generate(&mut self) {
        if self.cluster_vec.is_empty() {
            return;
        }
        loop {
            let current_cluster = self.cluster_vec.last().unwrap().clone();
            let offset = current_cluster as usize * 4;
            let block_offset = offset / BLOCK_SIZE;
            let offset_left = offset % BLOCK_SIZE;
            let block_id = self.fat1_offset / BLOCK_SIZE + block_offset;
            let next_cluster = get_block_cache(block_id, Arc::clone(&self.device))
                .read()
                .read(offset_left, |&cluster: &u32| cluster);
            if next_cluster >= END_OF_CLUSTER {
                break;
            } else {
                self.cluster_vec.push(next_cluster);
            };
        }
    }
    /// Shrink cluster_vec to new_size.
    pub(crate) fn truncate(&mut self, new_size: usize) {
        self.cluster_vec.truncate(new_size);
    }
}

//  整个 Fat 表的簇号从 2 开始, 0 和 1 为保留簇号, 0 表示无效簇号, 1 表示最后一个簇号,
//  在数据区以 cluster_size 为单位从 0 开始编号, 故根据 cluster_id 求出偏移时 cluster_id - 2
//  通过 bpb.first_data_sector() 可得到从磁盘0号扇区开始编号的数据区的第一个扇区号(距离磁盘0号扇区的扇区数)
pub struct FATManager {
    device: Arc<dyn BlockDevice>,
    recycled_cluster: VecDeque<u32>,
    fat1_offset: usize,
}

impl FATManager {
    pub fn open(fat_offset: usize, device: Arc<dyn BlockDevice>) -> Self {
        Self {
            device: Arc::clone(&device),
            recycled_cluster: VecDeque::new(),
            fat1_offset: fat_offset,
        }
    }
    // Only used for std test when creating fat32 file system.
    #[allow(unused)]
    pub fn new(fat_offset: usize, device: Arc<dyn BlockDevice>) -> Self {
        let fat = Self {
            device: Arc::clone(&device),
            recycled_cluster: VecDeque::new(),
            fat1_offset: fat_offset,
        };
        // Initialize FAT1 Table
        // 由于簇号从 2 开始, 现在将簇号 0, 1 的内容填充方便找到正确的簇(防止误操作)
        let block_id = fat.fat1_offset / BLOCK_SIZE;
        assert!(fat.fat1_offset % BLOCK_SIZE == 0);
        get_block_cache(block_id, Arc::clone(&device))
            .write()
            .modify(0, |buf: &mut [u32; 2]| {
                buf[0] = END_OF_CLUSTER;
                buf[1] = END_OF_CLUSTER;
            });

        fat
    }
    /// Given any valid cluster number N, return the sector number and offset of the entry for that cluster number in the FAT.
    ///
    // 给出 FAT 表的下标(clsuter_id_in_fat数据区簇号), 返回这个下标 (fat表的) 相对于磁盘的扇区数 (block_id) 与扇区内偏移
    pub fn cluster_id_pos(&self, index: u32) -> (usize, usize) {
        // When create a new file, the first cluster is 0.
        // We will use alloc_cluster() to allocate a new cluster for it.
        // The function alloc_cluster() which argrument of start_cluster is 0,
        // and this function will use function cluster_id_pos() eventually.
        // So `assert!(index >= 2)` is will panic.
        // Don't worry, in real fat32 file system image, function find_block_cluster() will find a valid cluster
        // assert!(index >= 2); // No need to check this
        let offset = index as usize * 4 + self.fat1_offset;
        let block_id = offset / BLOCK_SIZE;
        let offset_in_block = offset % BLOCK_SIZE;
        (block_id, offset_in_block)
    }
    // 从 start_from 开始找 在FAT表中找到空闲的簇
    fn find_blank_cluster(&self, start_from: u32) -> u32 {
        // 加 1 过滤已经分配的簇号 (该簇号还未初始值为EOC, 防止找到同样的簇号)
        let mut cluster = start_from + 1;
        let mut done = false;
        let mut buffer = [0u8; BLOCK_SIZE];
        loop {
            let (block_id, offset) = self.cluster_id_pos(cluster);
            get_block_cache(block_id, Arc::clone(&self.device))
                .read()
                .read(0, |buf: &[u8; BLOCK_SIZE]| {
                    buffer.copy_from_slice(buf);
                });
            for i in (offset..BLOCK_SIZE).step_by(4) {
                if read_le_u32(&buffer[i..i + 4]) == 0 {
                    done = true;
                    break;
                } else {
                    cluster += 1;
                }
            }
            if done {
                break;
            }
        }
        cluster & CLUSTER_MASK
    }
    pub fn get_blank_cluster(&mut self, start_from: u32) -> u32 {
        if let Some(cluster) = self.recycled_cluster.pop_front() {
            cluster & CLUSTER_MASK
        } else {
            self.find_blank_cluster(start_from)
        }
    }
    pub fn recycle(&mut self, cluster: u32) {
        self.recycled_cluster.push_back(cluster);
    }
    // Query the next cluster of the specific cluster
    //
    // 最后一个簇的值, next_cluster 可能等于 EOC
    pub fn get_next_cluster(&self, cluster: u32) -> Option<u32> {
        let (block_id, offset_in_block) = self.cluster_id_pos(cluster);

        let next_cluster: u32 = get_block_cache(block_id, Arc::clone(&self.device))
            .read()
            .read(offset_in_block, |&next_cluster: &u32| next_cluster);

        assert!(next_cluster >= 2);
        if next_cluster >= END_OF_CLUSTER {
            None
        } else {
            Some(next_cluster)
        }
    }
    // Set the next cluster of the specific cluster
    //
    // 在磁盘的FAT表中的簇号 cluster(offset) 处写入 cluster 的 value(下一个簇号)
    pub fn set_next_cluster(&self, cluster: u32, next_cluster: u32) {
        let (block_id, offset_in_block) = self.cluster_id_pos(cluster);
        get_block_cache(block_id, Arc::clone(&self.device))
            .write()
            .modify(offset_in_block, |value: &mut u32| {
                *value = next_cluster;
            });
    }
    // Get the ith cluster of a cluster chain
    pub fn get_cluster_at(&self, start_cluster: u32, index: u32) -> Option<u32> {
        let mut cluster = start_cluster;
        for _ in 0..index {
            let option = self.get_next_cluster(cluster);
            if let Some(c) = option {
                cluster = c
            } else {
                return None;
            }
        }
        Some(cluster & CLUSTER_MASK)
    }
    // Get the last cluster of a cluster chain
    pub fn cluster_chain_tail(&self, start_cluster: u32) -> u32 {
        let mut curr_cluster = start_cluster;
        // start cluster 是 fat 表中的 index, 从 2 开始有效
        assert!(curr_cluster >= 2);
        loop {
            let option = self.get_next_cluster(curr_cluster);
            if let Some(cluster) = option {
                curr_cluster = cluster
            } else {
                return curr_cluster & CLUSTER_MASK;
            }
        }
    }
    // Get all clusters of a cluster chain starting from the specified cluster
    pub fn get_all_cluster_id(&self, start_cluster: u32) -> Vec<u32> {
        let mut curr_cluster = start_cluster;
        let mut vec: Vec<u32> = Vec::new();
        loop {
            vec.push(curr_cluster & CLUSTER_MASK);
            let option = self.get_next_cluster(curr_cluster);
            if let Some(next_cluster) = option {
                curr_cluster = next_cluster;
            } else {
                return vec;
            }
        }
    }
    pub fn cluster_chain_len(&self, start_cluster: u32) -> u32 {
        let mut curr_cluster = start_cluster;
        let mut len = 0;
        loop {
            len += 1;
            let option = self.get_next_cluster(curr_cluster);
            if let Some(next_cluster) = option {
                curr_cluster = next_cluster;
            } else {
                return len;
            }
        }
    }
}
