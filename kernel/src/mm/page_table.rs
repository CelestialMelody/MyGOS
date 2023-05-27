//! Sv39 页表
//!
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
use super::{alloc_frame, FrameTracker};
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

// SV39 多级页表
#[derive(Debug)]
pub struct PageTable {
    /// 根节点的物理页号,作为页表唯一的区分标志
    root_ppn: PhysPageNum,
    /// 以 FrameTracker 的形式保存了页表所有的节点（包括根节点）所在的物理页帧
    /// 用以延长物理页帧的生命周期
    frames: Vec<FrameTracker>,
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
    /// `satp` 寄存器在 x64 上的布局：
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
            // 不需要重新生成节点，节点已经在原始多级页表中存在，同时存在在内存中
            frames: Vec::new(),
        }
    }

    /// 根据vpn查找对应页表项，如果在查找过程中发现无效页表则新建页表
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        // 当前节点的物理页号，最开始指向多级页表的根节点
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            // 通过 get_pte_array 将取出当前节点的页表项数组，并根据当前级页索引找到对应的页表项
            let pte = &mut ppn.as_pte_array()[*idx];
            if i == 2 {
                // 找到第三级页表，这个页表项的可变引用
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                // 发现页表项是无效的状态
                // 获取一个物理页帧
                let frame = alloc_frame().unwrap();
                // 用获取到的物理页帧生成新的页表项
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                // 将生成的页表项存入页表
                self.frames.push(frame);
            }
            // 切换到下一级页表（物理页帧）
            ppn = pte.ppn();
        }
        result
    }

    /// 根据vpn查找对应页表项，如果在查找过程中发现无效页表则直接返回 None 即查找失败
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
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
    /// 根据VPN找到第三级页表中的对应项，将 `PPN` 和 `flags` 写入到页表项
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        // 断言，保证新获取到的PTE是无效的（不是已分配的）
        assert!(!pte.is_valid(), "{:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    /// 删除一个虚拟页号到物理页号的映射
    ///
    /// 只需根据虚拟页号找到页表项，然后修改或者直接清空其内容即可
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "{:?} is invalid before unmapping", vpn);
        pte.clear();
    }

    /// 根据 vpn 查找页表项
    ///
    /// 调用 `find_pte` 来实现，如果能够找到页表项，那么它会将页表项拷贝一份并返回，否则就返回一个 `None`
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    /// 在当前多级页表中将虚拟地址转换为物理地址
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            //println!("translate_va:va = {:?}", va);
            let aligned_pa: PhysAddr = pte.ppn().into();
            //println!("translate_va:pa_align = {:?}", aligned_pa);
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();

            (aligned_pa_usize + offset).into()
        })
    }

    /// 按照 satp CSR 格式要求 构造一个无符号 64 位无符号整数，使得其分页模式为 SV39 ，且将当前多级页表的根节点所在的物理页号填充进去
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
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

    // only X+W+R can be set
    // return -1 if find no such pte
    // pub fn set_pte_flags(&mut self, vpn: VirtPageNum, flags: usize) -> isize {
    //     let idxs = vpn.indexes();
    //     let mut ppn = self.root_ppn;
    //     for i in 0..3 {
    //         let pte = &mut ppn.get_pte_array()[idxs[i]];
    //         if i == 2 {
    //             pte.set_pte_flags(flags);
    //             break;
    //         }
    //         if !pte.is_valid() {
    //             return -1;
    //         }
    //         ppn = pte.ppn();
    //     }
    //     0
    // }
}

// 可以将一个 u8 封装成一个标志位的集合类型，支持一些常见的集合运算
bitflags! {
    /// ### 页表项标志位
    /// |标志位|描述|
    /// |--|--|
    /// |`V(Valid)`|仅当位 V 为 1 时，页表项才是合法的；
    /// |`R(Read)` `W(Write)` `X(eXecute)`|分别控制索引到这个页表项的对应虚拟页面是否允许读/写/执行；
    /// |`U(User)`|控制索引到这个页表项的对应虚拟页面是否在 CPU 处于 U 特权级的情况下是否被允许访问；
    /// |`G`|暂且不理会；
    /// |`A(Accessed)`|处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被访问过；
    /// |`D(Dirty)`|处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被修改过
    #[derive(Copy, Clone, PartialEq, Eq)]
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
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
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    /// 验证页表项是否合法（V标志位是否为1）
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// 验证页表项是否可读（R标志位是否为1）
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// 验证页表项是否可写（W标志位是否为1）
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// 验证页表项是否可执行（X标志位是否为1）
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
    // only X+W+R can be set
    pub fn set_pte_flags(&mut self, flags: usize) {
        self.bits = (self.bits & !(0b1110 as usize)) | (flags & (0b1110 as usize));
    }

    pub fn set_flags(&mut self, flags: PTEFlags) {
        let new_flags: u8 = flags.bits().clone();
        self.bits = (self.bits & 0xFFFF_FFFF_FFFF_FF00) | (new_flags as usize);
    }

    pub fn set_cow(&mut self) {
        (*self).bits = self.bits | (1 << 9);
    }

    pub fn reset_cow(&mut self) {
        (*self).bits = self.bits & !(1 << 9);
    }

    pub fn is_cow(&self) -> bool {
        self.bits & (1 << 9) != 0
    }
}
