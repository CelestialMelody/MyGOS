# Bite The Disk

## 项目定位

具有详实文档和开发流程记录的多 CPU 支持内核开发项目，可用于教学参考或开发样例。

## 目标

使用 Rust 语言实现多 CPU 内核所需功能与结构，在学长的代码基础上逐步重写，逐步减少第三方依赖。使当前项目能作为教材或样例为后人提供参考

- 选择 Rust 的理由：
  - Rust 相对于 C/C++，在一定程度上解决了内存和并发的安全性问题，能够减少意料之外的错误
  - 小组成员有着一定的 Rust 使用经验，对 Rust 及其相关生态比较熟悉
  - Rust 提供了出色的模块化管理方案，其内置的数据结构支持也是我们考虑的因素之一
  - 相对 C/C++，Rust 的依赖管理和构建方式较为出色，自动化构建省去了不必要的麻烦
- 选择继承上届学长代码的原因
  - 所选学长代码参考于清华大学 rCore，小组成员多数参加过 rCore-Tutorial 的夏令营，有一定的从零编写内核代码经验，对已形成生态的 rCore 相关比较熟悉
  - 项目相关可以得到上届学长的支持答疑
  - 省去从零开始的麻烦，专注于实现自己的想法，巩固所学

## 预计产出

- 较高质量的代码
- 详实的文档支持
- 开发流程记录

## 进度

### 文件系统（90% debugging）

- [x] 依据 Fat32 文档，用 Rust 语言实现对 Fat32 格式镜像的读写支持
- [x] 对 lib 添加块缓存，提高性能
- [ ] ~~充实 lib，支持大多数 Fat 格式镜像的读写~~

### 内存管理

- [ ] 参考 Linux 实现应用地址空间，支持 mmap, brk, sbrk 等系统调用
- [ ] 参考 `buddy_system_allocator` 实现自己的内核堆地址分配器
- [ ] 支持堆内存溢出 panic handle
- [ ] 尝试基于 buddy 内存分配器实现一个简单的 slab 内存分配器

### 进程调度
- [ ] 实现简单的分时调度策略
- [ ] 简单支持优先级
- [ ] 支持多核调度，实现简单的调度方案

### IO

*暂时使用第三方库*

- [ ] 调整 `BlockDevice`，删去不必要的 `trait` 以适配高版本 VirtIOBlk


## 项目结构
| 目录      | 简述                                     |
| --------- | ---------------------------------------- |
| .vscode   | VSCode, RA, C/C++ 相关配置               |
| docs      | 项目相关文档                             |
| fat32     | FAT32 文件系统                           |
| misc      | 系统调用测例，util 脚本等                |
| os        | 内核源码，SBI                            |
| workspace | 用于做一下临时的挂载等任务，方便内核调试 |

## 区域赛相关文档

[点我前往](docs/oscomp_syscalls.md)



