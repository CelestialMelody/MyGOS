use super::file::File;
use crate::{mm::UserBuffer, task::suspend_current_and_run_next};
use alloc::{
    string::{String, ToString},
    sync::{Arc, Weak},
    vec::Vec,
};
use nix::Kstat;
use spin::Mutex;

pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<Mutex<PipeRingBuffer>>,
}
impl Pipe {
    /// Create the read end of a pipe.
    pub fn read_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }
    /// Create the write end of a pipe.
    pub fn write_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
enum RingBufferStatus {
    Full,
    Empty,
    Normal,
}

const RING_BUFFER_SIZE: usize = 4096;

pub struct PipeRingBuffer {
    /// Buffer memory block
    arr: [u8; RING_BUFFER_SIZE],
    /// Queue head, read
    head: usize,
    /// Queue tail, write
    tail: usize,
    /// Queue status
    status: RingBufferStatus,
    /// Save a weak reference count of its write end.
    /// Used to determine if all write ends of the pipe have been closed.
    write_end: Option<Weak<Pipe>>,
}

impl PipeRingBuffer {
    pub fn new() -> Self {
        Self {
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: RingBufferStatus::Empty,
            write_end: None,
        }
    }
    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }
    /// Write a byte to the tail of the pipe
    pub fn write_byte(&mut self, byte: u8) {
        self.status = RingBufferStatus::Normal;
        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = RingBufferStatus::Full;
        }
    }
    /// Read a byte from the head of the pipe
    pub fn read_byte(&mut self) -> u8 {
        self.status = RingBufferStatus::Normal;
        let c = self.arr[self.head];
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = RingBufferStatus::Empty;
        }
        c
    }
    /// Get the remaining readable length in the pipe
    pub fn available_read(&self) -> usize {
        if self.status == RingBufferStatus::Empty {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            self.tail + RING_BUFFER_SIZE - self.head
        }
    }
    /// Get the remaining writable length in the pipe
    pub fn available_write(&self) -> usize {
        if self.status == RingBufferStatus::Full {
            0
        } else {
            RING_BUFFER_SIZE - self.available_read()
        }
    }
    /// Check if all write ends of the pipe have been closed by the weak pointer of the pipe buffer write end
    pub fn all_write_ends_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }
}

/// Create a pipe and return the read end and write end of the pipe (read_end, write_end)
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    let buffer = Arc::new(Mutex::new(PipeRingBuffer::new()));
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
    buffer.lock().set_write_end(&write_end);

    (read_end, write_end)
}

impl File for Pipe {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn available(&self) -> bool {
        true
    }
    fn read_to_ubuf(&self, buf: UserBuffer) -> usize {
        #[cfg(feature = "time-tracer")]
        time_trace!("pipe_read");
        assert_eq!(self.readable(), true);
        let mut buf_iter = buf.into_iter();
        let mut read_size = 0usize;
        loop {
            let mut ring_buffer = self.buffer.lock();
            let loop_read = ring_buffer.available_read();
            if loop_read == 0 {
                if ring_buffer.all_write_ends_closed() {
                    return read_size;
                }
                drop(ring_buffer);
                if suspend_current_and_run_next() < 0 {
                    return read_size;
                }
                continue;
            }
            // read at most loop_read bytes
            for _ in 0..loop_read {
                if let Some(byte_ref) = buf_iter.next() {
                    unsafe {
                        *byte_ref = ring_buffer.read_byte();
                    }
                    read_size += 1;
                } else {
                    return read_size;
                }
            }
            return read_size;
        }
    }
    fn write_from_ubuf(&self, buf: UserBuffer) -> usize {
        assert_eq!(self.writable(), true);
        let mut buf_iter = buf.into_iter();
        let mut write_size = 0usize;
        loop {
            let mut ring_buffer = self.buffer.lock();
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                drop(ring_buffer);
                if suspend_current_and_run_next() < 0 {
                    return write_size;
                }
                continue;
            }
            for _ in 0..loop_write {
                if let Some(byte_ref) = buf_iter.next() {
                    ring_buffer.write_byte(unsafe { *byte_ref });
                    write_size += 1;
                } else {
                    return write_size;
                }
            }
        }
    }
    fn name(&self) -> String {
        "pipe".to_string()
    }
    fn offset(&self) -> usize {
        return 0;
    }
    fn seek(&self, _offset: usize) {
        return;
    }
    fn read_to_kspace(&self) -> Vec<u8> {
        assert_eq!(self.readable(), true);
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let mut ring_buffer = self.buffer.lock();
            let loop_read = ring_buffer.available_read();
            if loop_read == 0 {
                if ring_buffer.all_write_ends_closed() {
                    return buf;
                }
                drop(ring_buffer);
                if suspend_current_and_run_next() < 0 {
                    return buf;
                }
                continue;
            }
            for _ in 0..loop_read {
                buf.push(ring_buffer.read_byte());
            }
            return buf;
        }
    }
    fn write_from_kspace(&self, data: &Vec<u8>) -> usize {
        assert_eq!(self.writable(), true);
        let mut data_iter = data.into_iter();
        let mut write_size = 0usize;
        loop {
            let mut ring_buffer = self.buffer.lock();
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                drop(ring_buffer);
                if suspend_current_and_run_next() < 0 {
                    return write_size;
                }
                continue;
            }
            for _ in 0..loop_write {
                if let Some(data_ref) = data_iter.next() {
                    ring_buffer.write_byte(*data_ref);
                    write_size += 1;
                } else {
                    return write_size;
                }
            }
        }
    }
    fn file_size(&self) -> usize {
        core::usize::MAX
    }
    fn r_ready(&self) -> bool {
        let ring_buffer = self.buffer.lock();
        let loop_read = ring_buffer.available_read();
        loop_read > 0
    }
    fn w_ready(&self) -> bool {
        let ring_buffer = self.buffer.lock();
        let loop_write = ring_buffer.available_write();
        loop_write > 0
    }
    fn fstat(&self, _kstat: &mut Kstat) {
        // TODO: if needed to implement?
    }
    fn set_cloexec(&self) {
        let pipe = unsafe { (self as *const _ as *mut Self).as_mut().unwrap() };
        pipe.readable = false;
        pipe.writable = false;
    }
}
