//! The examples provided by virtio_drivers serve as references for implementation.
//! [reference](https://github.com/rcore-os/virtio-drivers/tree/master/examples/riscv)

use core::ptr::NonNull;
// use lazy_static::lazy_static;
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

use crate::mm::{
    acquire_kvmm, alloc_frame, dealloc_frame, FrameTracker, PageTable, PhysAddr as KPhysAddr,
    PhysPageNum, Step,
};
use alloc::vec::Vec;
use spin::Mutex;

// lazy_static! {
// static ref DMA_PADDR: Mutex<Vec<FrameTracker>> = Mutex::new(Vec::new());
// }

use spin::lazy::Lazy;
pub static DMA_PADDR: Lazy<Mutex<Vec<FrameTracker>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    #[no_mangle]
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let mut ppn_base = PhysPageNum(0);
        for i in 0..pages {
            let frame = alloc_frame().unwrap();
            if i == 0 {
                ppn_base = frame.ppn;
            }
            assert_eq!(frame.ppn.0, ppn_base.0 + i);
            DMA_PADDR.lock().push(frame);
        }
        let kpaddr: KPhysAddr = ppn_base.into();
        let paddr: PhysAddr = kpaddr.0;

        let vaddr = NonNull::new(paddr as _).unwrap();
        (paddr, vaddr)
    }
    #[no_mangle]
    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        let pa: KPhysAddr = paddr.into();
        let mut ppn_base: PhysPageNum = pa.into();
        for _ in 0..pages {
            dealloc_frame(ppn_base);
            ppn_base.step();
        }

        0
    }
    #[no_mangle]
    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as _).unwrap()
    }
    #[no_mangle]
    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        // Nothing to do, as the host already has access to all memory.
        virt_to_phys(vaddr)
    }
    #[no_mangle]
    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Nothing to do, as the host already has access to all memory and we didn't copy the buffer
        // anywhere else.
    }
}

fn virt_to_phys(vaddr: usize) -> PhysAddr {
    PageTable::from_token(acquire_kvmm().token())
        .translate_va(vaddr.into())
        .unwrap()
        .0
}
