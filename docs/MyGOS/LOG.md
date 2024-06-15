MyGOS

2024/06/14

- SD 卡驱动问题仍未解决。在设置 clk_control_reg 的 int_clk_en 为 1 后，int_clk_stable 仍未被设置为 1，可能与地址有关？虽然地址理论上是正确的，从 dtb 文件解析出的地址和官方 cv18x_pinout excel文件均指明地址无误，但 mygos 上无法使用 cv1811h-sd。驱动在 byteos 上可以使用，byteos 的地址空间设计与 mygos 不同，因此怀疑地址与之有关。
- 暂时跳过了SD卡驱动问题。为了在板卡上展示，临时实现了ramfs。选择将busybox打包到内核，而不是从SD卡中读取，以便在板卡上运行busybox。但相关方法仍存在问题，文件相关的部分系统调用与fat32关系较为紧密，比如获取目录项的系统调用，虽然依据fat32实现的系统调用方法正确，但在接入ramfs时发现之前实现文件系统相关系统调用与fat32关联度较高，因此在实现ramfs时需要配合fat32遗留的设计（感觉优化空间很大）。
- 实际上，项目可以在cv1812h上运行，这是满足移植硬件的基本要求的。但官方并未提供关于SD卡支持，且riscv社区没有为其实现硬件驱动相关库（K210与HiFive是有的），个人实现SD卡驱动较为困难。

2024/06/15
- 修复了部分 ramfs bug
- 修复了内核的 OpenFlags

补充说明

- 若需要从零开始了解/实现SD驱动，以下链接会有所帮助：
  - [SDHC](https://onlinedocs.microchip.com/oxy/GUID-A52628F4-6F6F-4C77-80CB-113A0C62DB75-en-US-6/GUID-9F1262D4-7170-4B9E-83EA-139A8DE15465.html) 非常完整的介绍，非常具有参考价值，但注意这是microchip的文档，可能与其他厂商有所不同。
  - [SD Card Formatter 5.0.2](https://www.sdcard.org/pdf/SD_CardFormatterUserManualJP.pdf)
  - [SD memory card formatter for Linux](https://www.sdcard.org/downloads/formatter/eula_linux/index.html)
