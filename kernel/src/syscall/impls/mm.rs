//! About syscall detail: https://man7.org/linux/man-pages/dir_section_2.html

use crate::mm::create_shm;
use crate::mm::MapPermission;
use crate::mm::PTEFlags;
use crate::mm::PageTable;
use crate::mm::{VPNRange, VirtAddr};
use crate::return_errno;
use crate::{
    consts::PAGE_SIZE,
    mm::remove_shm,
    task::{current_task, current_user_token},
};

use nix::ipc::{IPC_PRIVATE, IPC_RMID};
use nix::MmapFlags;
use nix::MmapProts;

use super::*;

// brk 214
pub fn sys_brk(brk: usize) -> Result {
    let task = current_task().unwrap();
    if brk == 0 {
        Ok(task.grow_proc(0) as isize)
    } else {
        let former_addr = task.grow_proc(0);
        let grow_size: isize = (brk - former_addr) as isize;
        Ok(current_task().unwrap().grow_proc(grow_size) as isize)
    }
}

// munmap 215
pub fn sys_munmap(addr: usize, length: usize) -> Result {
    let task = current_task().unwrap();
    Ok(task.munmap(addr, length) as isize)
}

// mmap 222
pub fn sys_mmap(
    addr: usize,
    length: usize,
    prot: usize,
    flags: usize,
    fd: isize,
    offset: usize,
) -> Result {
    if length == 0 {
        return Err(Errno::EINVAL);
    }

    let mut padding = PAGE_SIZE - length % PAGE_SIZE;
    if padding == PAGE_SIZE {
        padding = 0;
    }
    let length = length + padding;
    let task = current_task().unwrap();
    let fd_table = task.fd_table.read();
    let prot = MmapProts::from_bits(prot).expect("unsupported mmap prot");
    let flags = MmapFlags::from_bits(flags).expect("unsupported mmap flags");
    if !flags.contains(MmapFlags::MAP_ANONYMOUS)
        && (fd as usize >= fd_table.len() || fd_table[fd as usize].is_none())
    {
        return Err(Errno::EBADF);
    }

    drop(fd_table);

    let result_addr = task.mmap(addr, length, prot, flags, fd, offset);

    Ok(result_addr as isize)
}

// shmget 194
pub fn sys_shmget(key: usize, size: usize, shmflg: usize) -> Result {
    let size = (size + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE;
    assert!(size % PAGE_SIZE == 0);
    let mut new_key = key;
    if key == IPC_PRIVATE {
        new_key = create_shm(key, size, shmflg);
    } else {
        unimplemented!();
    }
    Ok(new_key as isize)
}

// shmctl 195
// TODO
pub fn sys_shmctl(key: usize, cmd: usize, _buf: *const u8) -> Result {
    if cmd == IPC_RMID {
        remove_shm(key);
    } else {
        unimplemented!();
    }
    Ok(0)
}

// shmat 196
// TODO
pub fn sys_shmat(key: usize, address: usize, _shmflg: usize) -> Result {
    let task = current_task().unwrap();
    let mut memory_set = task.memory_set.write();
    let address = if address == 0 {
        memory_set.shm_top
    } else {
        address
    };
    memory_set.attach_shm(key, address.into());
    drop(memory_set);
    Ok(address as isize)
}

// shmdt 197
pub fn sys_shmdt(address: usize) -> Result {
    let task = current_task().unwrap();
    let mut memory_set = task.memory_set.write();
    let nattch = memory_set.detach_shm(address.into());
    drop(memory_set);
    // detach_shm called when drop SharedMemoryTracker

    Ok(nattch as isize)
}

// mprotect 226
pub fn sys_mprotect(addr: usize, length: usize, prot: usize) -> Result {
    let token = current_user_token();
    let page_table = PageTable::from_token(token);

    let map_flags = (((prot & 0b111) << 1) + (1 << 4)) as u64;
    let map_perm = MapPermission::from_bits(map_flags).unwrap();
    // let pte_flags = PTEFlags::from_bits(map_perm.bits()).unwrap() | PTEFlags::V;
    let pte_flags = PTEFlags::from_map_permission(map_perm) | PTEFlags::V;

    let start_va = VirtAddr::from(addr);
    let end_va = VirtAddr::from(addr + length);
    let vpn_range = VPNRange::from_va(start_va, end_va);

    for vpn in vpn_range {
        if let Some(pte) = page_table.find_pte(vpn) {
            pte.set_flags(pte_flags);
        } else {
            let task = current_task().unwrap();
            let mut memory_set = task.memory_set.write();
            let mmap_start = memory_set.mmap_manager.mmap_start;
            let mmap_top = memory_set.mmap_manager.mmap_top;
            let mmap_perm = MmapProts::from_bits(prot).unwrap();
            let va: VirtAddr = vpn.into();
            if va >= mmap_start && va < mmap_top {
                memory_set
                    .mmap_manager
                    .mmap_map
                    .get_mut(&vpn)
                    .unwrap()
                    .set_prot(mmap_perm);
                continue;
            }
            return_errno!(Errno::EINVAL, "invalid address: {:?}", va);
        }
    }
    Ok(0)
}
