use alloc::{collections::BTreeMap, sync::Arc};

use crate::{
    consts::PAGE_SIZE,
    fs::File,
    mm::{
        address::Step, alloc_frame, page_table::PTEFlags, FrameTracker, PageTable, PhysPageNum,
        VPNRange, VirtAddr, VirtPageNum,
    },
};

use super::{MapPermission, MapType};

pub struct VmArea {
    pub area_type: VmAreaType,
    pub vpn_range: VPNRange,
    pub map_type: MapType,
    pub permission: MapPermission,
    pub file: Option<Arc<dyn File>>, // Mapped file.
    pub file_offset: usize,          // Offset of the mapped file in the file.
    pub frame_map: BTreeMap<VirtPageNum, FrameTracker>, // vpn -> frame_tracker
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum VmAreaType {
    UserHeap,
    UserStack,
    Elf,
    TrapContext,
    Shared, // Mmap
    KernelStack,
    KernelSpace,
}

impl VmArea {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        area_type: VmAreaType,
        permission: MapPermission,
        file: Option<Arc<dyn File>>,
        file_offset: usize,
    ) -> Self {
        Self {
            vpn_range: VPNRange::from_va(start_va, end_va),
            map_type,
            area_type,
            permission,
            frame_map: BTreeMap::new(),
            file,
            file_offset,
        }
    }

    pub fn from_another(another: &Self) -> Self {
        Self {
            vpn_range: another.vpn_range,
            frame_map: BTreeMap::new(),
            map_type: another.map_type,
            area_type: another.area_type,
            permission: another.permission,
            file: another.file.clone(),
            file_offset: another.file_offset,
        }
    }

    pub fn start_vpn(&self) -> VirtPageNum {
        self.vpn_range.get_start()
    }

    #[allow(unused)]
    pub fn end_vpn(&self) -> VirtPageNum {
        self.vpn_range.get_end()
    }

    pub fn inflate_pagetable(&mut self, page_table: &mut PageTable) {
        match self.map_type {
            MapType::Identical => {
                self.vpn_range.into_iter().for_each(|vpn| {
                    let ppn = PhysPageNum(vpn.0);
                    let flags = PTEFlags::from_bits(self.permission.bits()).unwrap();
                    page_table.map(vpn, ppn, flags);
                });
            }
            MapType::Framed => {
                self.vpn_range.into_iter().for_each(|vpn| {
                    let frame = alloc_frame().expect("out of memory");
                    let ppn = frame.ppn;
                    if self.frame_map.contains_key(&vpn) {
                        panic!("vm area overlap");
                    }
                    self.frame_map.insert(vpn, frame);
                    let flags = PTEFlags::from_bits(self.permission.bits()).unwrap(); // TODO premission and flags 统一
                    page_table.map(vpn, ppn, flags);
                });
            }
        }
    }

    /// Remove the mapping from the current logical segment to physical
    /// memory from the multi-level page table of the address space to
    /// which the incoming logical segment belongs.
    /// Write back to file (if map_perm includes the W permission).
    #[allow(unused)]
    pub fn write_back(&self, page_table: &mut PageTable) -> Result<(), ()> {
        if !self.permission.contains(MapPermission::W) {
            return Ok(());
        }
        if self.file.is_none() {
            return Ok(());
        }
        let file = self.file.as_ref().unwrap();
        if !file.writable() {
            return Ok(());
        }
        for vpn in self.vpn_range {
            match page_table.translate(vpn) {
                None => {}
                Some(pte) => {
                    if !pte.is_valid() {
                        continue;
                    }
                    let data = pte.ppn().as_bytes_array();
                    let offset =
                        (vpn.0 - self.vpn_range.get_start().0) * PAGE_SIZE + self.file_offset;
                    file.seek(offset);
                    file.write_from_kspace(&data.to_vec());
                }
            }
        }
        Ok(())
    }

    /// Remove the mapping from the current logical segment to physical
    /// memory from the multi-level page table of the address space to
    /// which the incoming logical segment belongs.
    pub fn erase_pagetable(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            page_table.unmap(vpn);
        }
    }

    pub fn copy_data(
        &mut self,
        page_table: &mut PageTable,
        data_start: usize,
        mut data_len: usize,
        mut page_offset: usize,
    ) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut offset: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let file = self.file.as_ref().unwrap();
        loop {
            let data = file.kernel_read_with_offset(data_start + offset, data_len.min(PAGE_SIZE));
            let data_slice = data.as_slice();

            let src = &data_slice[0..data_len.min(PAGE_SIZE - page_offset)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .as_bytes_array()[page_offset..page_offset + src.len()];
            dst.copy_from_slice(src);
            offset += PAGE_SIZE - page_offset;

            page_offset = 0;
            data_len -= src.len();
            if data_len == 0 {
                break;
            }
            current_vpn.step();
        }
    }

    pub fn lazy_map_vpn(&mut self, vpn: VirtPageNum, page_table: &mut PageTable) {
        self.map_one(page_table, vpn);
    }

    fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                if let Some(frame) = alloc_frame() {
                    ppn = frame.ppn;
                    self.frame_map.insert(vpn, frame);
                } else {
                    panic!("No more memory!");
                }
            }
        }
        let pte_flags = PTEFlags::from_bits(self.permission.bits()).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }
}
