use alloc::string::{String, ToString};

use crate::fs::file::File;
use crate::mm::UserBuffer;
use crate::sbi::console_getchar;
use crate::task::suspend_current_and_run_next;

pub struct Stdin;

impl File for Stdin {
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        false
    }
    fn available(&self) -> bool {
        true
    }

    fn read_to_ubuf(&self, mut user_buf: UserBuffer) -> usize {
        assert_eq!(user_buf.len(), 1);
        let mut c: i32;
        loop {
            c = console_getchar() as i32;
            if c <= 0 {
                suspend_current_and_run_next();
                continue;
            } else {
                break;
            }
        }
        let ch = c as u8;
        unsafe { user_buf.buffers[0].as_mut_ptr().write_volatile(ch) }

        1
    }

    fn write_from_ubuf(&self, _user_buf: UserBuffer) -> usize {
        panic!("Cannot write to stdin!");
    }

    fn name(&self) -> String {
        "Stdin".to_string()
    }

    fn offset(&self) -> usize {
        0
    }

    fn seek(&self, _offset: usize) {}

    fn file_size(&self) -> usize {
        usize::MAX
    }
    fn truncate(&self, _new_length: usize) {
        warn!("Fake truncate for Stdin");
    }
}
