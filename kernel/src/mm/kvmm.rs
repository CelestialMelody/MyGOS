use alloc::sync::Arc;
use spin::{Mutex, MutexGuard};

#[cfg(feature = "cvitex")]
use crate::boards::{MemRegion, MMIO, PHYSICAL_MEM_END};
#[cfg(feature = "qemu")]
use crate::boards::{MMIO, PHYSICAL_MEM_END};

use crate::mm::{
    vm_area::{VmArea, VmAreaType},
    MapPermission, MapType, MemorySet, VPNRange,
};

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss();
    fn ebss();
    fn skstack0();
    fn ekstack0();
    fn ekernel();
}

// lazy_static! {
//     // Kernel's virtual memory memory set.
//     static ref KERNEL_VMM: Arc<Mutex<MemorySet>> = Arc::new(Mutex::new({
//         let mut memory_set = MemorySet::new_bare();
//         memory_set.map_trampoline();
//         macro_rules! insert_kernel_vm_areas {
//             ($kvmm:ident,$($start:expr, $end:expr, $permission:expr, $file:expr, $page_offset:expr)*) => {
//                 $(
//                     $kvmm.insert(
//                         VmArea::new(
//                             ($start as usize).into(),
//                             ($end as usize).into(),
//                             MapType::Identical,
//                             VmAreaType::KernelSpace,
//                             $permission,
//                             $file,
//                             $page_offset,
//                         ),
//                         None
//                     );
//                 )*
//             };
//         }
//         insert_kernel_vm_areas! { memory_set,
//             stext,   etext,    MapPermission::R | MapPermission::X, None, 0
//             srodata, erodata,  MapPermission::R, None, 0
//             sdata,   edata,    MapPermission::R | MapPermission::W, None, 0
//             sbss,    ebss,     MapPermission::R | MapPermission::W, None, 0
//             ekernel, PHYSICAL_MEM_END,
//                 MapPermission::R | MapPermission::W, None, 0
//         }

//         // For MMIO(Memory mapped IO).
//         for &pair in MMIO {
//             insert_kernel_vm_areas!(memory_set,
//                 pair.0, pair.0+pair.1, MapPermission::R | MapPermission::W, None, 0);
//         }

//         memory_set
//     }));
// }

use spin::lazy::Lazy;
pub static KERNEL_VMM: Lazy<Arc<Mutex<MemorySet>>> = Lazy::new(|| {
    let mut memory_set = MemorySet::new_bare();
    memory_set.map_trampoline();
    macro_rules! insert_kernel_vm_areas {
        ($kvmm:ident,$($start:expr, $end:expr, $permission:expr, $file:expr, $page_offset:expr)*) => {
            $(
                $kvmm.insert_and_map(
                    VmArea::new(
                        ($start as usize).into(),
                        ($end as usize).into(),
                        MapType::Identical,
                        VmAreaType::KernelSpace,
                        $permission,
                        $file,
                        $page_offset,
                    ),
                    None
                );
            )*
        };
    }
    // TODO 单独取出 trampoline
    // 实际上 trampoline 这里也被映射了一次，不过影响不大
    // 相当于 vpn1(最高页面指定映射, map_trampoline) -> ppn1, vpn2(ppn1恒等映射, insert_kernel_vm_area) -> ppn1
    // page_table map 时是传入的 vpn, 通过 vpn 查找 ppn, 不同 vpn 映射同一个 ppn 并没有问题
    insert_kernel_vm_areas! { memory_set,
        stext,   etext,    MapPermission::R | MapPermission::X, None, 0
        srodata, erodata,  MapPermission::R, None, 0
        sdata,   edata,    MapPermission::R | MapPermission::W, None, 0
        sbss,    ebss,     MapPermission::R | MapPermission::W, None, 0
        ekernel, PHYSICAL_MEM_END,
            MapPermission::R | MapPermission::W, None, 0
    }

    // println!(
    //     "[kvmm] insert kernel region [text], start: {:#x}, end: {:#x}",
    //     stext as usize, etext as usize
    // );
    // println!(
    //     "[kvmm] insert kernel region [rodata], start: {:#x}, end: {:#x}",
    //     srodata as usize, erodata as usize
    // );
    // println!(
    //     "[kvmm] insert kernel region [data], start: {:#x}, end: {:#x}",
    //     sdata as usize, edata as usize
    // );
    // println!(
    //     "[kvmm] insert kernel region [bss], sbss: {:#x}, ebss: {:#x}, skstack0: {:#x}, ekstack0: {:#x}",
    //     sbss as usize, ebss as usize, skstack0 as usize, ekstack0 as usize
    // );
    // println!(
    //     "[kvmm] insert kernel region [else], start: {:#x}, end: {:#x}",
    //     ekernel as usize, PHYSICAL_MEM_END
    // );

    // For MMIO(Memory mapped IO).
    #[cfg(feature = "qemu")]
    for &pair in MMIO {
        insert_kernel_vm_areas!(
            memory_set,
            pair.0,
            pair.0 + pair.1,
            MapPermission::R | MapPermission::W,
            None,
            0
        );
    }

    #[cfg(feature = "cvitex")]
    {
        let mapped_region: MemRegion = MemRegion {
            start: stext as usize,
            end: PHYSICAL_MEM_END,
        };
        // println!("mapped_region: {:x?}", mapped_region);

        let mut mmio = MMIO.lock().clone();

        // 排序：按照 start 从小到大排序，再按照 end 从大到小排序
        mmio.sort_by(|a, b| {
            if a.start == b.start {
                b.end.cmp(&a.end)
            } else {
                a.start.cmp(&b.start)
            }
        });

        let mut mmio_map_region = mmio.clone();

        // 移除区域：(在mmio中查找符合条件的区域，在mmio_map_region中移除)
        // 1. 包含区域
        //    eg. reg1: (0x3000000, 0x3008000) reg2: (0x3002000, 0x3002008)
        //    reg1 包含 reg2，所以移除 reg2
        // 2. 同一页面的区域
        //    eg. reg1: (0x300a100, 0x300a200) 对应的页面为 0x300a100 / page_size(0x1000) = 0x300a, 0x300a200 / page_size(0x1000) = 0x300a
        //        reg2: (0x300a000, 0x300a100) 对应的页面为 0x300a100 / page_size(0x1000) = 0x300a, 0x300a100 / page_size(0x1000) = 0x300a
        //   reg1 和 reg2 同属于 0x300a 页面，所以移除 reg2

        for i in 0..mmio.len() {
            for j in (i + 1)..mmio.len() {
                if mmio[i].start <= mmio[j].start && mmio[i].end >= mmio[j].end {
                    mmio_map_region.retain(|x| x.start != mmio[j].start);
                } else if mmio[i].start / 0x1000 == mmio[j].start / 0x1000
                    && mmio[i].end / 0x1000 == mmio[j].end / 0x1000
                {
                    mmio_map_region.retain(|x| x.start != mmio[j].start);
                }
            }
        }

        mmio_map_region.iter().map(|x| {
            let start = x.start;
            let end = x.end;
            println!(
                "[kvmm] insert mmio region, start: {:#x}, end: {:#x}",
                start, end
            );
        });

        for pair in mmio_map_region.iter() {
            if pair.start <= mapped_region.start && pair.end >= mapped_region.end {
                continue;
            }

            println!(
                "[kvmm] insert mmio region, start: {:#x}, end: {:#x}",
                pair.start, pair.end
            );
            insert_kernel_vm_areas!(
                memory_set,
                pair.start,
                pair.end,
                MapPermission::R | MapPermission::W,
                None,
                0
            );
        }
        println!("except mapped_region, mapped mmio finnished");
    }
    // let token = memory_set.token();
    // println!("token: {:#x}", token);
    Arc::new(Mutex::new(memory_set))
});

pub fn acquire_kvmm<'a>() -> MutexGuard<'a, MemorySet> {
    KERNEL_VMM.lock()
}
