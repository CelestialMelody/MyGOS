pub const CLOCK_FREQ: usize = 1000_000;

pub const MMIO: &[(usize, usize)] = &[
    (0x0000_6000, 0x1000),     // Chip select   0x1000      0x0000_6FFF
    (0x0200_0000, 0x10000),    // CLINT         0x10000     0x0200_FFFF
    (0x0C00_0000, 0x4000000),  // PLIC          0x4000000   0x0FFF_FFFF
    (0x1000_0000, 0x1000),     // PRCI          0x1000      0x1000_0FFF
    (0x1001_0000, 0x1000),     // UART0         0x1000      0x1001_0FFF
    (0x1001_1000, 0x1000),     // UART1         0x1000      0x1001_1FFF
    (0x1004_0000, 0x1000),     // QSPI 0        0x1000      0x1004_0FFF
    (0x1004_1000, 0x1000),     // QSPI 1        0x1000      0x1004_1FFF
    (0x1005_0000, 0x1000),     // QSPI 2        0x1000      0x1005_0FFF
    (0x1006_0000, 0x1000),     // GPIO          0x1000      0x1006_0FFF
    (0x1008_0000, 0x1000),     // Pin control   0x1000      0x1008_0FFF
    (0x2000_0000, 0x10000000), // SPI 0         0x10000000  0x2FFF_FFFF
    (0x3000_0000, 0x10000000), // SPI 1         0x10000000  0x3FFF_FFFF
];

pub const PHYSICAL_MEM_END: usize = 0xC0000000;
