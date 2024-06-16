use buddy_system_allocator::LockedHeap;

use crate::consts::KERNEL_HEAP_SIZE;

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:#x?}", layout);
}

// static mut KERNEL_HEAP: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];
#[repr(align(4096))]
pub struct KernelHeap([u8; KERNEL_HEAP_SIZE]);

static mut KERNEL_HEAP: KernelHeap = KernelHeap([0; KERNEL_HEAP_SIZE]);

pub fn init_heap() {
    // let heap_addr = unsafe { KERNEL_HEAP.0.as_ptr() as usize };
    // let heap_end = heap_addr + KERNEL_HEAP_SIZE;
    // println!(
    //     "[kernel] heap starts at: {:#x}, ends at: {:#x}",
    //     heap_addr, heap_end
    // );
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(KERNEL_HEAP.0.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

#[allow(unused)]
pub fn heap_usage() {
    let usage_actual = HEAP_ALLOCATOR.lock().stats_alloc_actual();
    let usage_all = HEAP_ALLOCATOR.lock().stats_total_bytes();
    println!("[kernel] HEAP USAGE:{:?} {:?}", usage_actual, usage_all);
}
