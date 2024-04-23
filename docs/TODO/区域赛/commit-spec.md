# Commit specification

这只是个建议的 commit 信息规范，只要能言简意赅地写清楚，其实怎样都可以

## 导读

commit 信息用很多种格式，针对不同的代码增改，不同的格式各有其优缺点

commit 信息尽可能不用缩写，允许适度地使用常见的缩写形式

下面简单举例几种 commit 信息的写法（只是举例，优劣请自己体会）：

- feat: simple logger
- Fix duplicate declaration.
- fixes #65536; remove bug code.
- go/token: fix a typo

## 建议的前缀

- feat: 表示一个相对完整功能或特性的完成与应用
    e.g.

    ```shell
    feat: debug logging
    ```

- rm: 删除了哪些文件
    e.g.

    ```shell
    rm: src/consts.rs src/pid.rs
    ```

- rename: 文件重命名
    e.g.

    ```shell
    rename: sys_call -> syscall, kvmem -> kernel_mm
    ```

- fix: bug 修复
    e.g.

    ```shell
    fix: duplicate declaration
    ```

- chore: 项目配置和构建的调整，不涉及代码的改动
    e.g.

    ```shell
    chore: simplify build process
    ```

## 对于单个文件内部修改的情况

指定出修改后的文件，并简单概括修改的内容
    e.g.

```shell
src/task/kernel_stack.rs: rm unused structs
```

## 对于涉及到多个文件的修改，但并没有实现特定功能和 bug 修复的情况

1. 尽量不要使用模糊的写法，例如: update batch，这样写只能说明做了一定的更新，完全不知道目的是什么（自我评价.jpg）
2. 单次 commit 尽量有实际性的进展

下面举例一个可能的 commit 格式：

progress(memory management): tidy

## 其他可能的情况

### 增改相关文档

```shell
docs: add commit-spec.md
```

### 版本回退

```shell
revert(e49660)
```

### 同步代码

```shell
sync(master): update local proj
```

### 代码格式整理，如对齐等（只需给出大概的范围）

```shell
pretty(src/mm, src/trap)
```

### 项目重构或大幅改动

```shell
refactor: modularize
```

**注：句子末尾的`.`可加可不加，commit 信息尽量统一用英文概括**

有其他想法均可对该文档进行增补 :P
