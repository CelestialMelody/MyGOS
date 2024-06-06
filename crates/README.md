# crates

这里放着的是 MyGOS 依赖的库

- cv1811h-sd

  适用于 CV1811H 的 SD 卡驱动(CV1812H 也可以使用)

- fat32

  解析读写 FAT32 镜像

- libd

  适用于 MyGOS 的用户库

- nix

  我们以库的方式将 POSIX 要求的数据结构分离出来，这些数据结构可能并非内核必须的。这样做以达到简化内核结构的目的。

- path

    一个简单的（绝对）路径处理库

- simple-sync

  包含简单的自旋锁、懒加载等

- sync_cell

  应对 `rustc` 检查的全局 Cell

- time_tracer

  利用riscv时钟计时的时间追踪器
