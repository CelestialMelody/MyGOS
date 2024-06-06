# busybox 预加载

由于大部分测试需要使用 busybox，为了避免多次解析 elf、从零创建地址空间等问题，我们采用了类似于加载initproc的方法。具体而言，我们将 busybox 预加载到内核中，并保存 load_elf 获取的信息。每次执行busybox时，我们直接使用保存的 load_elf 信息，并通过写时拷贝来创建所需的 busybox 进程的地址空间，更快速地创建 busybox。



```rust
// kernel/src/task/initproc/mod.rs
pub static ref BUSYBOX: RwLock<Busybox> = RwLock::new({
    extern "C" {
        fn busybox_entry();
        fn busybox_tail();
    }
    let entry = busybox_entry as usize;
    let tail = busybox_tail as usize;
    let siz = tail - entry;

    let busybox = unsafe { core::slice::from_raw_parts(entry as *const u8, siz) };
    let path = AbsolutePath::from_str("/busybox0");

    let inode = fs::open(path, OpenFlags::O_CREATE, CreateMode::empty()).expect("busybox0 create failed");
    inode.write_all(&busybox.to_owned());

    let bb = Arc::new(TaskControlBlock::new(inode.clone()));
    inode.delete();
    Busybox {
        inner: bb,
    }
});

pub static mut ONCE_BB_ENTRY: usize = 0;
pub static mut ONCE_BB_AUX: Vec<AuxEntry> = Vec::new();

pub struct Busybox {
    inner: Arc<TaskControlBlock>,
}

impl Busybox {
    pub fn elf_entry_point(&self) -> usize {
        unsafe { ONCE_BB_ENTRY }
    }
    pub fn aux(&self) -> Vec<AuxEntry> {
        unsafe { ONCE_BB_AUX.clone() }
    }
    pub fn memory_set(&self) -> MemorySet {
        let mut write = self.inner.memory_set.write();
        MemorySet::from_copy_on_write(&mut write)
    }
}
```