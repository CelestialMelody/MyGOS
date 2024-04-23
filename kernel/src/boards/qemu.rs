//! 针对 qemu 的相关参数

/// RTC (Real time clock)
pub const CLOCK_FREQ: usize = 12500000;

/// MMIO on Qemu of VirtIO.
pub const MMIO: &[(usize, usize)] = &[(0x10001000, 0x1000)];

pub const PHYSICAL_MEM_END: usize = 0x88000000; // 128 MiB
