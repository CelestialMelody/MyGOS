use crate::timer::get_time;
use crate::{mm::PhysAddr, task::current_task};
use alloc::{collections::BTreeMap, vec::Vec};

use nix::{CreateMode, SharedMemoryIdentifierDs};
use spin::Mutex;
// lazy_static! {
//     pub static ref SHM_MANAGER: Mutex<SharedMemoryManager> = Mutex::new(SharedMemoryManager::new());
// }
use spin::lazy::Lazy;
pub static SHM_MANAGER: Lazy<Mutex<SharedMemoryManager>> =
    Lazy::new(|| Mutex::new(SharedMemoryManager::new()));

pub struct SharedMemoryManager {
    shm_areas: BTreeMap<usize, SharedMemoryArea>,
}
pub struct SharedMemoryArea {
    shmid_ds: SharedMemoryIdentifierDs,
    buffer: Vec<u8>,
}
pub struct SharedMemoryTracker {
    pub key: usize,
}
impl SharedMemoryTracker {
    pub fn new(key: usize) -> Self {
        attach_shm(key);
        Self { key }
    }
}
impl Drop for SharedMemoryTracker {
    fn drop(&mut self) {
        detach_shm(self.key);
    }
}
impl SharedMemoryManager {
    pub fn new() -> Self {
        Self {
            shm_areas: BTreeMap::new(),
        }
    }
    pub fn create(&mut self, key: usize, size: usize, shmflags: usize) -> usize {
        let key = if key == 0 {
            if self.shm_areas.is_empty() {
                1
            } else {
                self.shm_areas.last_key_value().unwrap().0 + 1
            }
        } else {
            key
        };
        let pid = current_task().unwrap().pid();
        let perm = CreateMode::from_bits((shmflags & 0o777) as u32).unwrap();
        let shmid_ds = SharedMemoryIdentifierDs {
            shm_perm: perm,
            shm_size: size,
            shm_atime: 0,
            shm_dtime: 0,
            shm_ctime: get_time(),
            shm_cpid: pid,
            shm_lpid: 0,
            shm_nattch: 0,
        };
        let buffer: Vec<u8> = vec![0 as u8; size];
        let shm_area = SharedMemoryArea { shmid_ds, buffer };
        assert!(self.shm_areas.get(&key).is_none());
        self.shm_areas.insert(key, shm_area);
        key
    }
    pub fn attach(&mut self, key: usize) {
        let pid = current_task().unwrap().pid();
        let shm_area = &mut self.shm_areas.get_mut(&key).unwrap();
        shm_area.shmid_ds.shm_atime = get_time();
        shm_area.shmid_ds.shm_lpid = pid;
        shm_area.shmid_ds.shm_nattch += 1;
    }
    pub fn detach(&mut self, key: usize) {
        let pid = current_task().unwrap().pid();
        let shm_area = &mut self.shm_areas.get_mut(&key).unwrap();
        shm_area.shmid_ds.shm_dtime = get_time();
        shm_area.shmid_ds.shm_lpid = pid;
        shm_area.shmid_ds.shm_nattch -= 1;
    }
    pub fn remove(&mut self, key: usize) {
        let shm_area = &mut self.shm_areas.get_mut(&key).unwrap();
        if shm_area.shmid_ds.shm_nattch == 0 {
            println!("shm remove!");
            self.shm_areas.remove(&key);
        };
    }
    pub fn get_address_and_size(&self, key: usize) -> (PhysAddr, usize) {
        let shm_area = &self.shm_areas.get(&key).unwrap();
        let size = shm_area.shmid_ds.shm_size;
        ((shm_area.buffer.as_ptr() as usize).into(), size)
    }
    pub fn get_nattch(&self, key: usize) -> usize {
        let shm_area = &self.shm_areas.get(&key).unwrap();
        shm_area.shmid_ds.shm_nattch
    }
}

pub fn create_shm(key: usize, size: usize, perm: usize) -> usize {
    SHM_MANAGER.lock().create(key, size, perm)
}
pub fn attach_shm(key: usize) {
    SHM_MANAGER.lock().attach(key);
}
pub fn detach_shm(key: usize) {
    SHM_MANAGER.lock().detach(key);
}
pub fn remove_shm(key: usize) {
    SHM_MANAGER.lock().remove(key);
}

pub fn shm_get_address_and_size(key: usize) -> (PhysAddr, usize) {
    SHM_MANAGER.lock().get_address_and_size(key)
}
pub fn shm_get_nattch(key: usize) -> usize {
    SHM_MANAGER.lock().get_nattch(key)
}
