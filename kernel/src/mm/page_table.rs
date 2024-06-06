//! Sv39 Page table.
//!
//! ```text
//!                    +--------+--------+--------+
//!  stap      offset: | VPN[2] | VPN[1] | VPN[0] |
//!    |               +--------+--------+--------+
//!    |                    |
//!    +--> +--------+      |
//!         |        |      |
//!         +--------+ <----+
//!         |  PTE   | -------> +--------+      +----------+
//!         +--------+          |        |      |          |
//!         |        |          +--------+      +----------+      +------------+
//!         +--------+          |        |  ··· | leaf PTE | ---> |FrameTracker|
//!         |        |          +--------+      +----------+      +------------+
//!         +--------+          |        |      |          |
//!            物理页            +--------+      +----------+
//!                             |        |      |          |
//!                             +--------+      +----------+
//! ```

use super::address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::permission::MapPermission;
use super::{alloc_frame, FrameTracker};
use alloc::vec::Vec;
use bitflags::*;
use nix::MmapProts;

#[derive(Debug)]
pub struct PageTable {
    /// The root of current pagetable.
    root_ppn: PhysPageNum,
    pub frames: Vec<FrameTracker>,
}

impl PageTable {
    /// 新建一个 `PageTable`
    pub fn new() -> Self {
        let frame = alloc_frame().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame], // 将新获取到的物理页帧存入向量
        }
    }

    /// 通过 `satp` 获取对应的多级页表
    ///
    /// `satp` 寄存器在 x64 上的布局:
    /// ```text
    ///    64     60             44                   0
    ///     +------+--------------+-------------------+
    ///     | MODE |     ASID     |        PPN        |
    ///     +------+--------------+-------------------+
    ///         4         16                44
    /// ```
    ///
    /// **hint**: 物理页号位宽 44 bits
    pub fn from_token(satp: usize) -> Self {
        Self {
            // 取satp的前44位作为物理页号
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            // 不需要重新生成节点, 节点已经在原始多级页表中存在, 同时存在在内存中
            frames: Vec::new(),
        }
    }

    /// 根据vpn查找对应页表项, 如果在查找过程中发现无效页表则新建页表
    pub fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        // 当前节点的物理页号, 最开始指向多级页表的根节点
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            // 通过 get_pte_array 将取出当前节点的页表项数组, 并根据当前级页索引找到对应的页表项
            let pte = &mut ppn.as_pte_array()[*idx];
            if i == 2 {
                // 找到第三级页表, 这个页表项的可变引用
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                // 发现页表项是无效的状态
                // 获取一个物理页帧
                let frame = alloc_frame().unwrap();
                // 用获取到的物理页帧生成新的页表项
                // *pte = PageTableEntry::new(frame.ppn, "VAD".into());
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                // 将生成的页表项存入页表
                self.frames.push(frame);
            }
            // 切换到下一级页表(物理页帧)
            ppn = pte.ppn();
        }
        result
    }

    /// 根据vpn查找对应页表项, 如果在查找过程中发现无效页表则直接返回 None 即查找失败
    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.as_pte_array()[*idx];
            if !pte.is_valid() {
                return None;
            }
            if i == 2 {
                result = Some(pte);
                break;
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 建立一个虚拟页号到物理页号的映射
    ///
    /// 根据VPN找到第三级页表中的对应项, 将 `PPN` 和 `flags` 写入到页表项
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        // 断言, 保证新获取到的PTE是无效的(不是已分配的)
        assert!(!pte.is_valid(), "{:#x?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    /// 删除一个虚拟页号到物理页号的映射
    ///
    /// 只需根据虚拟页号找到页表项, 然后修改或者直接清空其内容即可
    pub fn unmap(&self, vpn: VirtPageNum) {
        if let Some(pte) = self.find_pte(vpn) {
            assert!(pte.is_valid(), "{:?} is invalid before unmapping", vpn);
            pte.clear();
        }
    }

    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    /// 在当前多级页表中将虚拟地址转换为物理地址
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            let aligned_pa: PhysAddr = pte.ppn().into();
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();

            (aligned_pa_usize + offset).into()
        })
    }

    /// A token indicating a valid memory set. In riscv, it's actually the S-mode's register `satp`
    pub fn token(&self) -> usize {
        0b1000 << 60 | self.root_ppn.0
    }

    pub fn set_cow(&mut self, vpn: VirtPageNum) {
        self.find_pte_create(vpn).unwrap().set_cow();
    }

    pub fn reset_cow(&mut self, vpn: VirtPageNum) {
        self.find_pte_create(vpn).unwrap().reset_cow();
    }

    pub fn set_flags(&mut self, vpn: VirtPageNum, flags: PTEFlags) {
        self.find_pte_create(vpn).unwrap().set_flags(flags);
    }

    pub fn remap_cow(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, former_ppn: PhysPageNum) {
        let pte = self.find_pte_create(vpn).unwrap();
        *pte = PageTableEntry::new(ppn, pte.flags() | PTEFlags::W);
        ppn.as_bytes_array()
            .copy_from_slice(former_ppn.as_bytes_array());
    }
}

/// Mask
#[cfg(feature = "qemu")]
pub const PTEFLAGS_MASK: usize = 0b11111_11111;

#[cfg(feature = "cvitex")]
pub const PTEFLAGS_MASK: usize = 0xF800_0000_0000_03FF;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct PTEFlags: u64 {
        /// 如果该位置零, 则当前 [`PTE`] 的其他位将失去其应有的意义, 具体意义由软件决定
        ///
        /// 换言之, 如果 MMU 转换过程中遇到 `!contains(PTEFlags::V)` 的情况, 则会引发 Page Fault
        const V = 1 << 0;

        /// 该 [`PTE`] 指向的物理页是否可读
        const R = 1 << 1;
        /// 该 [`PTE`] 指向的物理页是否可写
        const W = 1 << 2;
        /// 该 [`PTE`] 指向的物理页是否可执行
        const X = 1 << 3;

        /// 该 [`PTE`] 指向的物理页在用户态是否可以访问
        const U = 1 << 4;

        /// 该 [`PTE`] 指向的物理页是否被标记为全局页
        const G = 1 << 5;
        /// 该 [`PTE`] 指向的物理页是否被访问过
        const A = 1 << 6;
        /// 该 [`PTE`] 指向的物理页是否被写过
        const D = 1 << 7;

        /// 页表项指向的物理页帧是否需要写时复制
        const COW = 1 << 8; // RSW 1 << 8, 1 << 9

        #[cfg(feature = "cvitex")]
        const SO = 1 << 63;
        #[cfg(feature = "cvitex")]
        const C = 1 << 62;
        #[cfg(feature = "cvitex")]
        const B = 1 << 61;
        #[cfg(feature = "cvitex")]
        const SEC = 1 << 59;
    }
}

impl PTEFlags {
    pub fn from_map_permission(permission: MapPermission) -> Self {
        let mut flags = PTEFlags::V;
        if permission.contains(MapPermission::R) {
            flags |= PTEFlags::R;
        }
        if permission.contains(MapPermission::W) {
            flags |= PTEFlags::W;
        }
        if permission.contains(MapPermission::X) {
            flags |= PTEFlags::X;
        }
        if permission.contains(MapPermission::U) {
            flags |= PTEFlags::U;
        }
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::R) | flags.contains(PTEFlags::X) {
            flags = flags.union(PTEFlags::A)
        }
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::W) {
            flags = flags.union(PTEFlags::D)
        }
        flags
    }

    pub fn from_map_ports(prots: MmapProts) -> Self {
        let mut flags = PTEFlags::empty();
        if prots.contains(MmapProts::PROT_READ) {
            flags |= PTEFlags::R;
        }
        if prots.contains(MmapProts::PROT_WRITE) {
            flags |= PTEFlags::W;
        }
        if prots.contains(MmapProts::PROT_EXEC) {
            flags |= PTEFlags::X;
        }
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::R) | flags.contains(PTEFlags::X) {
            flags = flags.union(PTEFlags::A)
        }
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::W) {
            flags = flags.union(PTEFlags::D)
        }
        flags
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    /// 从一个物理页号 `PhysPageNum` 和一个页表项标志位 `PTEFlags` 生成一个页表项 `PageTableEntry` 实例
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        let mut flags = flags;
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::R) | flags.contains(PTEFlags::X) {
            flags = flags.union(PTEFlags::A)
        }
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::W) {
            flags = flags.union(PTEFlags::D)
        }
        #[cfg(feature = "cvitex")]
        // 内核和设备地址空间所设置的 PTE 额外属性值不同
        if flags.contains(PTEFlags::G) && ppn.in_memory() {
            PageTableEntry {
                bits: ppn.0 << 10 | flags.union(PTEFlags::C).union(PTEFlags::B).bits() as usize,
            }
        } else if flags.contains(PTEFlags::G) && ppn.in_device() {
            PageTableEntry {
                bits: ppn.0 << 10 | flags.union(PTEFlags::SO).bits() as usize,
            }
        } else {
            PageTableEntry {
                bits: ppn.0 << 10 | flags.union(PTEFlags::C).bits() as usize,
            }
        }
        #[cfg(feature = "qemu")]
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits() as usize,
        }
    }
    /// 将页表项清零
    pub fn clear(&mut self) {
        self.bits = 0;
    }
    /// 从页表项读取物理页号
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1_usize << 44) - 1)).into()
    }

    pub fn flags(&self) -> PTEFlags {
        let mut flags = PTEFlags::from_bits((self.bits & PTEFLAGS_MASK) as u64).unwrap();
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::R) | flags.contains(PTEFlags::X) {
            flags.union(PTEFlags::A)
        }
        #[cfg(feature = "cvitex")]
        if flags.contains(PTEFlags::W) {
            flags.union(PTEFlags::D)
        }
        flags
    }

    pub fn is_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }

    pub fn readable(&self) -> bool {
        self.flags().contains(PTEFlags::R)
    }

    pub fn writable(&self) -> bool {
        self.flags().contains(PTEFlags::W)
    }

    pub fn executable(&self) -> bool {
        self.flags().contains(PTEFlags::X)
    }

    pub fn set_flags(&mut self, flags: PTEFlags) {
        self.bits = (self.bits & 0xFFFF_FFFF_FFFF_FC00) | (flags.bits() as usize);
    }

    pub fn set_cow(&mut self) {
        (*self).bits = self.bits | (1 << 8);
    }

    pub fn reset_cow(&mut self) {
        (*self).bits = self.bits & !(1 << 8);
    }

    pub fn is_cow(&self) -> bool {
        self.flags().contains(PTEFlags::COW)
    }
}
