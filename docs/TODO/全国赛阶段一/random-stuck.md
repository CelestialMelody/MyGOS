## 内核在执行用户程序时随机卡死

内核在运行的时候总是会不知何时卡死，底层原因是持续触发时钟中断



```rust
// 时间片到了
Trap::Interrupt(Interrupt::SupervisorTimer) => {
    set_next_trigger(); // 主要是这里
    suspend_current_and_run_next();
}
```

## 初步解决

我们将设置下一个时钟中断放置在了 `suspend_current_and_run_next` 之前，导致可能因为后者执行
时间过长而使用户态一直处于时钟中断触发状态，至于为什么会在 RV64 上一直触发中断，可以参阅 RV 的特权级手册:

> Platforms provide a real-time counter, exposed as a memory-mapped machine-mode read-write
> register, mtime. mtime must increment at constant frequency, and the platform must provide a
> mechanism for determining the period of an mtime tick. The mtime register will wrap around if
> the count overflows.
>
> The mtime register has a 64-bit precision on all RV32 and RV64 systems. Platforms provide a 64-
> bit memory-mapped machine-mode timer compare register (mtimecmp). A machine timer interrupt
> becomes pending whenever mtime contains a value greater than or equal to mtimecmp, treating the
> values as unsigned integers. The interrupt remains posted until mtimecmp becomes greater than
> mtime (typically as a result of writing mtimecmp). The interrupt will only be taken if interrupts
> are enabled and the MTIE bit is set in the mie register.

由于 `suspend_current_and_run_next` 执行的时间超过了一个时间片的长度，导致其返回用户态进程时，
`mtime` 的值已经大于了 `set_next_trigger` 设置的时间点，由上文可得，如果 `mtime` 大于等于
`mtimecmp`(即 `set_next_trigger` 设置的值)，并且 `mie` 为使能状态，那么时钟中断会一直处于触发状态.

而我们的内核 `mie` 一直处于使能状态，所以 S 态的时钟中断会持续在用户态发生(S 态中断不会打断同级与
更高特权级代码的执行)，导致用户态毫无进展，而我们内核的引导程序 `initproc` 会一直等待卡死用户进程
变为僵尸态，所以造成了内核执行流的卡死.

解决办法:

简单调整下位置

```rust
Trap::Interrupt(Interrupt::SupervisorTimer) => {
    suspend_current_and_run_next();
    set_next_trigger();
}
```

## 但这样真的对吗?

不对，因为会导致用户态程序卡死整个内核的执行流

一个致命的缺点是，用户态的程序需要第一次运行后才能正确的获取时钟中断，不然只能等轮回一边后才可能正确让出

当前的逻辑是:

RustSBI 完成初始化后，在 `meow`(没错，这是我们 Rust 代码的 ENTRYPOINT)，中初步设定一个时钟中断


```rust
#[cfg(not(feature = "multi_harts"))]
#[no_mangle]
pub fn meow() -> ! {
    if hartid!() == 0 {
        init_bss();
        unsafe { set_fs(FS::Dirty) }
        lang_items::setup();
        logging::init();
        mm::init();
        trap::init();
        trap::enable_stimer_interrupt();
        trap::set_next_trigger();
        fs::init();
        task::add_initproc();
        task::run_tasks();
    } else {
        loop {}
    }

    unreachable!("main.rs/meow: you should not be here!");
}
```

这是第一个问题，我们原本想的是，这个时钟中断会在第一用户态程序运行时发生，但是有可能它在
`fs::init()` 或者 `task::add_initproc()` 中已经发生了，这会导致一进入用户态程序就发生中断，这和我们
预期的不一样.

而且，陷入中断后，除非使失能 `mie`，或者再次 `set_next_trigger()`(又或者 `mtime` 发生回环)，
否则将一直处于中断触发的状态

而这之后切换的用户进程都会遇到中断而直接返回，直到运行到第一个用户进程(其实应该是引导程序 `initproc`)，
在下面 `suspend_current_and_run_next` 真正意义上的返回后，重新设置下一个中断时间点，这才能让 OS 内核
所有的用户进程进入正常的运行流.

```rust
Trap::Interrupt(Interrupt::SupervisorTimer) => {
    suspend_current_and_run_next();
    set_next_trigger();
}
```

而这种时间上的开销显然是没必要的，所以我们根据所有用户进程都会通过 `trap_return` 返回用户态这一点，
将 `set_next_trigger` 设置在了 `trap_return` 中，同时判断当前进程是否是因为时间片耗尽而导致的
trap 返回:

```rust
Trap::Interrupt(Interrupt::SupervisorTimer) => {
    suspend_current_and_run_next();
}

// ...
#[no_mangle]
pub fn trap_return() -> ! {
    // ...

    if is_time_intr_trap() {
        set_next_trigger();
    }
    
    // ...
}

/// 是否是由于时间片耗尽导致的 trap
fn is_time_intr_trap() -> bool {
    let scause = scause::read();
    scause.cause() == Trap::Interrupt(scause::Interrupt::SupervisorTimer)
}
```

## 最终结果

- 我们消除了一个严重的 bug: **内核在执行用户程序时随机卡死**
- 删去了 `meow` 中不利于系统鲁棒性的代码

TODO
考虑以下情况, 当一个进程因为耗尽时间片而让出执行流, 切换回一个因为在内核态阻塞而让出执行流的
另外一个进程的时候(内核态让出 `suspend` 可能是因为读取了一个空的管道等原因), 由于我们没有对
scause 等寄存器进行保存, 所以当前这个由于时间片耗尽让出而恢复执行的内核态进程所关联的寄存器
其实是那个因为时间片耗尽而让出的进程的相关寄存器的值.

类似的, 当一个进程通过 `suspend` 让出执行流, 若让出给了一个之前因为时间片中断而让出执行流
的进程, 则当前这个因为时间片耗尽而让出的进程的 scause 相关寄存器值也已经不是自己的值

所以通过在 `trap_return` 通过当前的 scause 寄存器值来判断当前进程是否应该再次赋予新的
时间片的做法是错误的

另一种情况, 在由于时间片耗尽而让出的 `suspend` 之后直接设置新的时间片, 若在内核态执行了一
系列耗时操作导致时间片提前用尽, 则会使用户态程序一直处于时间片耗尽的状态而触发中断, 最终会
导致该用户态进程永远无法退出(在此用户态程序被 wait 的情况下等), 造成系统死锁

如果通过在 TaskControlBlock 中新加入一个字段 trap_cause 来保存和恢复 scause 则可以解决
这个问题, 但是如果内核在 trap_return 时执行的操作异常耗时(通常是错误的逻辑等), 那么会造成
系统性能问题, 而由于系统的"正确的"运行而造成的性能损耗相对于前边由于时间片问题导致内核锁死
来说更难排查, 所以目前只是在 TaskControlBlock 添加了字段并更新了逻辑, 但是并未使能

      inner.trap_cause = Some(scause);

之后再考虑处理这个问题