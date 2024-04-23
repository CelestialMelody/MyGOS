mod address;
mod frame_allocator;
mod kernel_heap_allocator;
mod kvmm;
mod memory_set;
mod mmap;
mod page_table;
mod permission;
mod shared_memory;
mod user_buffer;
mod vm_area;
pub use address::*;
pub use frame_allocator::*;
pub use kvmm::*;
pub use memory_set::*;
pub use mmap::*;
pub use page_table::*;
pub use permission::*;
pub use shared_memory::*;
pub use user_buffer::*;
pub use vm_area::*;

use crate::{boards::init_mmio, consts::PAGE_SIZE, task::current_task};
use alloc::{string::String, vec::Vec};
use core::{cmp::min, mem::size_of};
use riscv::register::satp;

/// Initialize kernel's frame allocator and enable MMU.
pub fn init() {
    init_mmio();
    println!("MMIO initialized");

    init_frame_allocator();

    println!("Frame allocator initialized");
    enable_mmu();
    println!("MMU enabled");
}

pub fn init_kernel_heap_allocator() {
    kernel_heap_allocator::init_heap();
}

pub fn init_frame_allocator() {
    frame_allocator::init();
}

pub fn enable_mmu() {
    satp::write(acquire_kvmm().token());
    unsafe { core::arch::asm!("sfence.vma") } // Refresh MMU's TLB
}

/// Get a bytes buffer [`Vec`] from user's memory set.
pub fn translated_bytes_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = VirtAddr::from(ptr as usize);
    let end = VirtAddr::from(start.0 + len);
    let mut v = Vec::new();
    let task = current_task().unwrap();
    while start < end {
        let mut vpn = start.floor();
        let ppn = match page_table.translate(vpn) {
            Some(pte) => pte.ppn(),
            None => {
                if task.check_lazy(start) != 0 {
                    info!("hart {} check lazy failed", hartid!());
                    panic!("check lazy error");
                }
                page_table.translate(vpn).unwrap().ppn()
            }
        };
        vpn.step();

        // Avoid page crossing.
        let in_page_end_va: VirtAddr = min(vpn.into(), end);
        if in_page_end_va.page_offset() == 0 {
            v.push(&mut ppn.as_bytes_array()[start.page_offset()..]);
        } else {
            v.push(&mut ppn.as_bytes_array()[start.page_offset()..in_page_end_va.page_offset()]);
        }
        start = in_page_end_va.into();
    }
    v
}

/// Get a translated [`String`] from user's memory set.
#[warn(deprecated)]
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

/// Get a reference T from user's memory set.
#[warn(deprecated)]
pub fn translated_ref<T>(token: usize, ptr: *const T) -> &'static T {
    let offset = ptr as usize % PAGE_SIZE;
    assert!(
        PAGE_SIZE - offset >= size_of::<T>(),
        "cross-page access from translated_ref"
    );
    let page_table = PageTable::from_token(token);
    page_table
        .translate_va(VirtAddr::from(ptr as usize))
        .unwrap()
        .as_ref()
}

/// Get a mutable reference T from user's memory set.
#[warn(deprecated)]
pub fn translated_mut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let offset = ptr as usize % PAGE_SIZE;
    assert!(
        PAGE_SIZE - offset >= size_of::<T>(),
        "cross-page access from translated_mut"
    );
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .as_mut()
}

/// Copy data from `src` from memory set indicated by the given token into `dst` in kernel's memory set.
pub fn copyin<T>(token: usize, dst: &mut T, src: *const T) {
    let src_buffer = translated_bytes_buffer(token, src as *const u8, core::mem::size_of::<T>());

    let dst_slice = unsafe {
        core::slice::from_raw_parts_mut(dst as *mut T as *mut u8, core::mem::size_of::<T>())
    };

    let mut start_byte = 0;
    let mut index = 0;
    loop {
        let src_slice = &src_buffer[index];
        index += 1;
        let src_slice_len = src_slice.len();
        dst_slice[start_byte..start_byte + src_slice_len].copy_from_slice(src_slice);
        start_byte += src_slice_len;
        if src_buffer.len() == index {
            break;
        }
    }
}

/// Copy data from `src` out of kernel memory set into `dst` which lives in the given
/// memory set indicated by the given `token`.
pub fn copyout<T>(token: usize, dst: *mut T, src: &T) {
    let mut dst_buffer =
        translated_bytes_buffer(token, dst as *const u8, core::mem::size_of::<T>());

    let src_slice = unsafe {
        core::slice::from_raw_parts(src as *const T as *const u8, core::mem::size_of::<T>())
    };
    let mut index = 0;

    let mut start_byte = 0;
    loop {
        let dst_slice = &mut dst_buffer[index];
        index += 1;
        let dst_slice_len = dst_slice.len();
        dst_slice.copy_from_slice(&src_slice[start_byte..start_byte + dst_slice_len]);
        start_byte += dst_slice_len;
        if dst_buffer.len() == index {
            break;
        }
    }
}
