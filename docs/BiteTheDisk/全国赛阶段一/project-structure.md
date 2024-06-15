## 项目结构(rust-workspace不能使用



随着开发的进行，我们需要的适配和封装的数据结构越来越多，其中大部分与我们的内核本体关系并没有那么紧密，
所以我们将这部分结构，如用于的引导程序、FAT32、Linux 相关数据结构放在了项目根目录中的 `crates` 里

Rust 本身是支持多个 `crates` 构成的一个 `workspace`，这些 `crates` 直接可以相互引用，但是由于我们使用
了 `.cargo/config.toml` 来配置 rustc，所以 `workspace` 并不能为我们所有 (因为目前 `workspace`
不支持在 `workspace` 中读取 `.cargo/config.toml`)

## 使用 Git Submodule 管理测例

与区域赛不同，全国赛的测例数目较多，如果一旦发生更新构建起来也相对麻烦

基于 Git Submodule 我们可以方便隔离当前 Git 仓库，做到依赖的隔离与同步

就当前的实际环境来说:

```shell
git submodule add https://github.com/oscomp/testsuits-for-oskernel.git testsuits
```

上面的作用是将 testsuits-for-oskernel.git clone 到本地的 testsuits 文件夹中，后者会自动创建

当重新拉取项目仓库时:

```shell
git submodule init
git submodule update
```

就可以重新拉取 `testsuits` 中，仓库的内容了

## 项目目录树

```
.
├── Makefile
├── README.md
├── crates
│   ├── fat32/     ---- FAT32 读写库
│   ├── libd/      ---- libc 的~~后继者(划掉)~~ initproc，内核自动加载的第一个用户程序
│   ├── nix/       ---- Linux 相关数据结构
│   └── sync_cell/ ---- 实现了 Sync 的，具有内部可变性的 RefCell
├── docs/
├── kernel/
│   ├── Makefile
│   ├── build.rs   ---- 用于监控相关文件，如 `crates/libd/bin/initproc.rs`，发生变化时重新编译
│   ├── cargo
│   │   └── config.toml
│   ├── linkerld
│   │   └── linker.ld
│   ├── src
│   │   ├── boards
│   │   │   └── qemu.rs ---- 平台相关参数
│   │   ├── console.rs
│   │   ├── consts.rs
│   │   ├── drivers
│   │   ├── entry.S
│   │   ├── error.rs
│   │   ├── fs/
│   │   ├── logging.rs
│   │   ├── macros
│   │   │   ├── hsm.rs
│   │   │   ├── mod.rs
│   │   │   ├── on_boot.rs
│   │   │   └── profile.rs ---- 用于打印某段代码运行时间的宏
│   │   ├── main.rs
│   │   ├── mm
│   │   ├── sbi.rs
│   │   ├── syscall/
│   │   ├── task/
│   │   ├── timer.rs
│   │   └── trap/
│   ├── target/ ---- 构建产物
│   └── vendor/ ---- 所有第三方依赖的本地归档
├── testsuits/ ---- 通过 Git Submodule 内联的官方测例
└── workspace ---- 用于中间过程构建内核运行所需测例

1187 directories, 9162 files

```
