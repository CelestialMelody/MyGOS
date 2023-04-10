//! # 物理页帧管理器
//! `os/src/mm/frame_allocator.rs`
//! ## 实现功能
//! ```
//! pub struct FrameTracker
//! FrameTracker::new(ppn: PhysPageNum) -> Self
//!
//! pub struct StackFrameAllocator
//! type FrameAllocatorImpl = StackFrameAllocator
//! // 全局物理页帧管理器
//! pub static ref FRAME_ALLOCATOR: Mutex<FrameAllocatorImpl>
//!
//! pub fn init_frame_allocator()
//! pub fn frame_alloc() -> Option<FrameTracker>
//! // 回收工作在FrameTracker生命周期结束时由编译器发起，故为私有
//! fn frame_dealloc(ppn: PhysPageNum)
//! ```
//!
use super::{PhysAddr, PhysPageNum};
use crate::config::PHYS_END;
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use spin::Mutex;

/// ### 物理页帧
/// 借用RAII思想，在通过物理页号创建的时候初始化物理页帧
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    /// 通过物理页号创建一个物理页帧的结构体，创建时初始化内存空间
    pub fn new(ppn: PhysPageNum) -> Self {
        // 物理页清零
        let bytes_array = ppn.as_bytes_mut();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

// 当变量生命周期结束被编译器回收的时候执行 drop()
impl Drop for FrameTracker {
    // 当一个 FrameTracker 生命周期结束被编译器回收的时候，我们需要将它控制的物理页帧回收掉
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

/// 物理页帧管理器
trait FrameAllocator {
    /// 新建一个实例，在使用前需要初始化
    fn new() -> Self;
    /// 从空闲物理页中分配一个物理页
    fn alloc(&mut self) -> Option<PhysPageNum>;
    /// 回收物理页
    fn dealloc(&mut self, ppn: PhysPageNum);
}

/// ### 栈式物理页帧管理器
/// - `current`:空闲内存的起始物理页号
/// - `end`:空闲内存的结束物理页号
/// - `recycled`:以后入先出的方式保存被回收的物理页号
///
/// ```
/// StackFrameAllocator::init(&mut self, l: PhysPageNum, r: PhysPageNum)
/// ```
pub struct StackFrameAllocator {
    /// 空闲内存的起始物理页号
    current: usize,
    /// 空闲内存的结束物理页号
    end: usize,
    /// 以后入先出的方式保存被回收的物理页号
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    /// ### 初始化栈式物理页管理器
    /// - `l`:空闲内存起始页号
    /// - `r`:空闲内存结束页号
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}
impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: vec![],
        }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        // 首先检查栈 recycled 内有没有之前回收的物理页号，如果有的话直接弹出栈顶并返回
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        }
        // 空间满返回 None
        else if self.current == self.end {
            None
        }
        // 否则就返回最低的物理页号
        else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // 验证物理页号有效性，PPN大于已分配的最高内存或已释放栈中存在这个物理页号
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // 回收，压栈
        self.recycled.push(ppn);
    }
}

/// 物理页帧管理器实例类型
type FrameAllocatorImpl = StackFrameAllocator;

lazy_static! {
    /// ### 物理页帧管理器实例
    /// - 全局变量，管理除内核空间外的内存空间
    pub static ref FRAME_ALLOCATOR: Mutex<FrameAllocatorImpl> =
         Mutex::new(FrameAllocatorImpl::new());
}

/// ### 初始化物理页帧管理器
/// - 物理页帧范围
///     - 对 `ekernel` 物理地址上取整获得起始物理页号
///     - 对 `MEMORY_END` 物理地址下取整获得结束物理页号
pub fn init() {
    extern "C" {
        fn ekernel();
    }
    FRAME_ALLOCATOR.lock().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(PHYS_END).floor(),
    );
}

/// 分配物理页帧
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc().map(FrameTracker::new)
}

/// 回收物理页帧
pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.lock().dealloc(ppn);
}
