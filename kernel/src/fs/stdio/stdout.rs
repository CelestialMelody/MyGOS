use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use nix::Kstat;

use crate::{fs::File, mm::UserBuffer};

pub struct Stdout;

impl File for Stdout {
    fn readable(&self) -> bool {
        false
    }
    fn writable(&self) -> bool {
        true
    }
    fn available(&self) -> bool {
        true
    }
    fn read_to_ubuf(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot read from stdout!");
    }
    fn write_from_ubuf(&self, user_buf: UserBuffer) -> usize {
        for buffer in user_buf.buffers.iter() {
            print!("{}", core::str::from_utf8(*buffer).unwrap());
        }
        user_buf.len()
    }

    fn name(&self) -> String {
        "Stdout".to_string()
    }

    fn write_from_kspace(&self, data: &Vec<u8>) -> usize {
        let buffer = data.as_slice();
        print!("{}", core::str::from_utf8(buffer).unwrap());

        data.len()
    }
    fn set_cloexec(&self) {}
    fn fstat(&self, _kstat: &mut Kstat) {
        warn!("Fake fstat for Stdout");
    }
}
