//! VirtAddr Abstraction

use super::PageTableEntry;
#[cfg(feature = "cvitex")]
use crate::boards::{PHYSICAL_MEM_BEGIN, PHYSICAL_MEM_END};
use crate::consts::PAGE_SIZE;
use core::fmt::Debug;

pub const IN_PAGE_OFFSET: usize = 0xc;

/// Physical address width of Sv39.
const PA_WIDTH_SV39: usize = 56;
/// Virtual address width of Sv39.
const VA_WIDTH_SV39: usize = 39;
/// Physical page number width of Sv39.
const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - IN_PAGE_OFFSET;
/// Virtual page number width of Sv39.
const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - IN_PAGE_OFFSET;

macro_rules! derive_wrap {
    ($($type_def:item)*) => {
        $(
            #[repr(C)]
            #[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
            $type_def
        )*
    };
}

derive_wrap! {
    pub struct PhysAddr(pub usize);
    pub struct VirtAddr(pub usize);
    pub struct PhysPageNum(pub usize);
    pub struct VirtPageNum(pub usize);
}

macro_rules! gen_into_usize {
    ($($addr_type:ident)*) => {
        $(
            impl From<$addr_type> for usize {
                fn from(value: $addr_type) -> Self {
                    value.0
                }
            }
        )*
    };
}

gen_into_usize! {
    PhysAddr
    PhysPageNum
    VirtPageNum
}

macro_rules! gen_from_usize {
    ($($addr_type:ident, $offset:expr)*) => {
        $(
            impl From<usize> for $addr_type {
                fn from(value: usize) -> Self {
                    Self(value & ((1 << $offset) - 1))
                }
            }
        )*
    };
}
impl From<usize> for VirtAddr {
    fn from(value: usize) -> Self {
        if value >> VA_WIDTH_SV39 == 0 {
            Self(value & ((1 << VA_WIDTH_SV39) - 1))
        } else {
            Self((value & ((1 << VA_WIDTH_SV39) - 1)) | (usize::MAX & !((1 << VA_WIDTH_SV39) - 1)))
        }
    }
}
impl From<VirtAddr> for usize {
    fn from(value: VirtAddr) -> Self {
        value.0
    }
}

gen_from_usize! {
    PhysAddr,    PA_WIDTH_SV39
    PhysPageNum, PPN_WIDTH_SV39
    VirtPageNum, VPN_WIDTH_SV39
}

macro_rules! mk_convertion_bridge {
    ($($from:ident <=> $into:ident)*) => {
        $(
            impl From<$from> for $into {
                fn from(value: $from) -> Self {
                    assert!(value.is_aligned(), "{:?} is not page aligned", value);
                    value.floor()
                }
            }

            impl From<$into> for $from {
                fn from(value: $into) -> Self {
                    Self(value.0 << IN_PAGE_OFFSET)
                }
            }
        )*
    };
}

mk_convertion_bridge! {
    PhysAddr <=> PhysPageNum
    VirtAddr <=> VirtPageNum
}

impl VirtAddr {
    /// Calculate the virtual page number from the virtual address (rounded down)
    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }
    /// Calculate the virtual page number from the virtual address (rounded up)
    pub fn ceil(&self) -> VirtPageNum {
        VirtPageNum((PAGE_SIZE - 1 + self.0) / PAGE_SIZE)
    }
    /// Get the page offset from the virtual address (the low 12 bits of the virtual address)
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    /// Judge whether the virtual address is aligned with the page size
    pub fn is_aligned(&self) -> bool {
        self.page_offset() == 0
    }
}

impl PhysAddr {
    /// Calculate the physical page number from the physical address (rounded down)
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }
    /// Calculate the physical page number from the physical address (rounded up)
    pub fn ceil(&self) -> PhysPageNum {
        PhysPageNum((PAGE_SIZE - 1 + self.0) / PAGE_SIZE)
    }
    /// Get the page offset from the physical address (the low 12 bits of the physical address)
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    /// Judge whether the physical address is aligned with the page size
    pub fn is_aligned(&self) -> bool {
        self.page_offset() == 0
    }
    /// Get an immutable slice of size T
    pub fn as_ref<T>(&self) -> &'static T {
        unsafe { (self.0 as *const T).as_ref().unwrap() }
    }
    /// Get a mutable slice of size T
    pub fn as_mut<T>(&self) -> &'static mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }
}

impl VirtPageNum {
    /// Take out the three-level page index of the virtual page number and return it in order from high to low
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3).rev() {
            idx[i] = vpn & 511; // 取出低9位
            vpn >>= 9;
        }
        idx
    }
}

impl PhysPageNum {
    /// According to its own PPN, take out the page table item array of the current node
    pub fn as_pte_array(&self) -> &'static mut [PageTableEntry] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut PageTableEntry, 512) }
    }
    /// Returns a mutable reference to a byte array that can be used to access data on a physical page frame in bytes
    pub fn as_bytes_array(&self) -> &'static mut [u8] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut u8, 4096) }
    }
    /// Get a mutable reference to a type T data that is exactly placed at the beginning of a physical page frame
    pub fn as_mut<T>(&self) -> &'static mut T {
        let pa: PhysAddr = (*self).into();
        unsafe { (pa.0 as *mut T).as_mut().unwrap() }
    }
    /// Get an immutable reference to a type T data that is exactly placed at the beginning of a physical page frame
    pub fn as_ref<T>(&self) -> &'static T {
        let pa: PhysAddr = (*self).into();
        unsafe { (pa.0 as *const T).as_ref().unwrap() }
    }
}

#[cfg(feature = "cvitex")]
pub struct PPNRange {
    start: usize,
    end: usize,
}

#[cfg(feature = "cvitex")]
pub static MEMORY_RANGE: PPNRange = PPNRange {
    start: PHYSICAL_MEM_BEGIN / PAGE_SIZE,
    end: PHYSICAL_MEM_END / PAGE_SIZE,
};

#[cfg(feature = "cvitex")]
impl PhysPageNum {
    pub fn in_memory(&self) -> bool {
        MEMORY_RANGE.start <= self.0 && self.0 < MEMORY_RANGE.end
    }

    pub fn in_device(&self) -> bool {
        if self.in_memory() {
            return false;
        }
        use crate::boards::MMIO;
        let mmio = MMIO.lock();
        mmio.iter()
            .any(|region| region.start <= self.0 && self.0 < region.end)
    }
}

/// Virtual page number range, is a left closed and right open interval
#[derive(Copy, Clone, Debug)]
pub struct VPNRange {
    start: VirtPageNum,
    end: VirtPageNum,
}
impl VPNRange {
    #[allow(unused)]
    pub fn from_vpn(start: VirtPageNum, end: VirtPageNum) -> Self {
        assert!(start <= end, "start {:?} > end {:?}!", start, end);
        Self { start, end }
    }
    pub fn from_va(start_va: VirtAddr, end_va: VirtAddr) -> Self {
        let start = start_va.floor();
        let end = end_va.ceil();
        assert!(start <= end, "start {:?} > end {:?}!", start, end);

        Self { start, end }
    }
    pub fn get_start(&self) -> VirtPageNum {
        self.start
    }
    pub fn get_end(&self) -> VirtPageNum {
        self.end
    }
}

impl IntoIterator for VPNRange {
    type Item = VirtPageNum;
    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            next: self.start,
            end: self.end,
        }
    }
}

pub struct IntoIter<T> {
    next: T,
    end: T,
}
impl<T> Iterator for IntoIter<T>
where
    T: PartialEq + Step,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            None
        } else {
            Some(self.next.step())
        }
    }
}
pub trait Step {
    /// Add up self and return the previous value.
    fn step(&mut self) -> Self;
}
impl Step for VirtPageNum {
    fn step(&mut self) -> Self {
        let current = self.clone();
        self.0 += 1;
        current
    }
}
impl Step for PhysPageNum {
    fn step(&mut self) -> Self {
        let current = self.clone();
        self.0 += 1;
        current
    }
}
