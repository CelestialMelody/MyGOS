我们使用 FrameTracker 来管理物理页帧的生命周期（必定不可为其派生 Clone/Copy Trait），
故 FrameTracker 的所有操作伴随着其自身引用计数的追踪。
而引用计数器在分配器 StackFrameAllocator 中。需要好好分析对引用计数器的操作。
未来的改进或许可以交给 Rust 提供的 Arc 来管理引用计数。
对于从 rCore-tutorial 学习的同学可能发现 rCore 在 StackFrameAllocator 中并未引入引用计数器，
实际上我们使用引用计数器是为了实现 CopyOnWrite 机制，而 rCore-tutorial 中并未实现 CopyOnWrite 机制。
当然手动维护引用计数器来实现 CopyOnWrite 机制并非是必须的，很多优秀的队伍也都实现了 CopyOnWrite 机制，
可以参考他们的实现。