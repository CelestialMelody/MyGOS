//!  Block device under VirtIO.

use super::virtio_impl::HalImpl;
use core::ptr::NonNull;

use fat32::{BlockDevice, BLOCK_SIZE};
// use crate::fat32::{BlockDevice, BLOCK_SIZE};

use spin::Mutex;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::mmio::{MmioTransport, VirtIOHeader},
};

const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock(Mutex<VirtIOBlk<HalImpl, MmioTransport>>);

unsafe impl Send for VirtIOBlock {}
unsafe impl Sync for VirtIOBlock {}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, blk_id: usize, buf: &mut [u8]) {
        assert_eq!(buf.len(), BLOCK_SIZE);
        self.0.lock().read_blocks(blk_id, buf).unwrap();
    }

    fn write_block(&self, blk_id: usize, buf: &[u8]) {
        self.0.lock().write_blocks(blk_id, buf).unwrap();
    }
}

// Refer to the examples provided by virtio_drivers for implementation.
// [reference](https://github.com/rcore-os/virtio-drivers/tree/master/examples/riscv)

impl VirtIOBlock {
    pub fn new() -> Self {
        let header = NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap();
        let blk = match unsafe { MmioTransport::new(header) } {
            Err(e) => {
                panic!("Error creating VirtIO MMIO transport: {}", e)
            }
            Ok(transport) => VirtIOBlk::<HalImpl, MmioTransport>::new(transport)
                .expect("failed to create blk driver"),
        };
        Self(Mutex::new(blk))
    }
}
