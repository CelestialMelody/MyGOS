extern crate alloc;

use super::{get_blk_device, BlkDriver, DeviceType, DeviceWapper, Driver};
use alloc::sync::Arc;

// use cv1811h_sd::clk_enable;
use super::cv1811h_sd::{self, clk_enable, init};

use fat32::BlockDevice;
// use crate::fat32::BlockDevice;

use fdt::node::FdtNode;
use spin::Mutex;

pub struct CvSd;

impl CvSd {
    pub fn new() -> Self {
        CvSd
    }
}

// need?
pub struct CvSdWrapper(Mutex<CvSd>);
// pub struct CvSdWrapper(CvSd);

impl CvSdWrapper {
    pub fn new() -> Self {
        CvSdWrapper(Mutex::new(CvSd::new()))
        // CvSdWrapper(CvSd::new())
    }
}

unsafe impl Sync for CvSd {}
unsafe impl Send for CvSd {}

impl Driver for CvSd {
    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn get_id(&self) -> &str {
        "cvitek,sd"
    }

    fn get_device_wrapper(self: Arc<Self>) -> DeviceWapper {
        DeviceWapper::BLOCK(self.clone())
    }
}

impl BlkDriver for CvSd {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        cv1811h_sd::clk_enable(true);
        cv1811h_sd::read_block(block_id as _, buf).expect("can't read block by using CvSd");
        cv1811h_sd::clk_enable(false);
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        cv1811h_sd::clk_enable(true);
        cv1811h_sd::write_block(block_id as _, buf).expect("can't write block by using CvSd");
        cv1811h_sd::clk_enable(false);
    }
}

impl BlockDevice for CvSdWrapper {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.0.lock().read_block(block_id, buf);
        // self.0.read_block(block_id, buf);
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.0.lock().write_block(block_id, buf);
        // self.0.write_block(block_id, buf);
    }
}

pub fn init_blk_driver() {
    println!("test 1.1");
    cv1811h_sd::init().expect("init with err");
    println!("Initailize blk device");
}
#[cfg(feature = "cvitex")]
pub type BlockDeviceImpl = CvSdWrapper;
