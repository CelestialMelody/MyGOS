pub const USER_STACK_SIZE: usize = 4096 * 2048;
pub const KERNEL_STACK_SIZE: usize = 4096 * 32; // 128K

pub const USER_HEAP_SIZE: usize = 4096 * 30000;
pub const KERNEL_HEAP_SIZE: usize = 4096 * 8192 * 2; // 32M

pub const PAGE_SIZE: usize = 0x1000;

pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
pub const SIGNAL_TRAMPOLINE: usize = TRAMPOLINE - PAGE_SIZE;

pub const TRAP_CONTEXT_BASE: usize = SIGNAL_TRAMPOLINE - PAGE_SIZE;

pub const USER_STACK_BASE: usize = 0xf000_0000;

pub const THREAD_LIMIT: usize = 4096 * 2;

pub const MMAP_BASE: usize = 0x6000_0000;

pub const SHM_BASE: usize = 0x7000_0000;

pub const LINK_BASE: usize = 0x2000_0000;

pub const FD_LIMIT: usize = 1024;

// pub const NCPU: usize = 2;
