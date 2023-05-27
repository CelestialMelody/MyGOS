pub mod address; // 地址数据类型
mod frame_allocator; // 物理页帧管理器
pub mod kernel_vmm;
pub mod memory_set; // 地址空间模块
mod page_table; // 页表
mod user_buffer;
mod vma; // 虚拟内存地址映射空间

use core::mem::size_of;

pub use address::*;
use alloc::{string::String, vec::Vec};
pub use frame_allocator::{alloc_frame, dealloc_frame, FrameTracker};
pub use memory_set::{MapPermission, MemorySet};
pub use page_table::{PageTable, PageTableEntry};
use riscv::register::satp;
pub use user_buffer::{UserBuffer, UserBufferIterator};
pub use vma::*;

use crate::{consts::PAGE_SIZE, kernel_token, task::current_task};

use self::address::Step;

/// 内存管理子系统的初始化
pub fn init() {
    init_frame_allocator();
    enable_mmu();
}

pub fn init_frame_allocator() {
    frame_allocator::init();
}

pub fn enable_mmu() {
    satp::write(kernel_token!());
    unsafe { core::arch::asm!("sfence.vma") } // 刷新 MMU 的 TLB
}

/// 以向量的形式返回一组可以在内存空间中直接访问的字节数组切片
///
/// |参数|描述|
/// |--|--|
/// |`token`|某个应用地址空间的 token|
/// |`ptr`|应用地址空间中的一段缓冲区的起始地址
/// |`len`|应用地址空间中的一段缓冲区的长度
pub fn translated_bytes_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn: PhysPageNum;
        match page_table.translate(vpn) {
            Some(_ppn) => ppn = _ppn.ppn(),
            None => {
                if current_task().unwrap().check_lazy(start_va, true) != 0 {
                    panic!("check lazy error");
                }
                ppn = page_table.translate(vpn).unwrap().ppn();
            }
        }
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.as_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.as_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

/// 从内核地址空间之外的某个应用的用户态地址空间中拿到一个字符串
///
/// 针对应用的字符串中字符的用户态虚拟地址，查页表，找到对应的内核虚拟地址，逐字节地构造字符串，直到发现一个 \0 为止
pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    loop {
        let ch: u8 = *(page_table
            .translate_va(VirtAddr::from(va))
            .unwrap()
            .as_mut());
        if ch == 0 {
            break;
        } else {
            string.push(ch as char);
            va += 1;
        }
    }
    string
}

/// 根据 多级页表token (satp) 和 虚拟地址 获取大小为 T 的空间的不可变切片
pub fn translated_ref<T>(token: usize, ptr: *const T) -> &'static T {
    let offset = ptr as usize % PAGE_SIZE;
    assert!(PAGE_SIZE - offset >= size_of::<T>(), "cross-page access");
    let page_table = PageTable::from_token(token);
    page_table
        .translate_va(VirtAddr::from(ptr as usize))
        .unwrap()
        .as_ref()
}

/// 根据 多级页表token (satp) 和 虚拟地址 获取大小为 T 的空间的切片
pub fn translated_mut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let offset = ptr as usize % PAGE_SIZE;
    assert!(PAGE_SIZE - offset >= size_of::<T>(), "cross-page access");
    //println!("into translated_refmut!");
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .as_mut()
}
