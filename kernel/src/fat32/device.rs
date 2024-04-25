use core::any::Any;
use core::marker::{Send, Sync};

pub trait BlockDevice: Send + Sync + Any {
    fn read_block(&self, blk_id: usize, buf: &mut [u8]);
    fn write_block(&self, blk_id: usize, buf: &[u8]);
}
