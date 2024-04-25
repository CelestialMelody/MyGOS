//! BIOS Parameter Block (BPB)
//! See Microsoft FAT32 File System Specification 1.03
// 布局如下:
//      引导扇区 - 保留扇区 - FAT1 - FAT2 - 数据区
// 1. 保留扇区包括引导扇区, 引导扇区包括 BPB 和 FSInfo
// 2. FAT1 起始地址 = 保留扇区数 * 扇区大小
// 3. 文件分配表区共保存了两个相同的文件分配表, 因为文件所占用的存储空间 (簇链) 及空闲空间的管理都是通过FAT实现的, 保存两个以便第一个损坏时, 还有第二个可用
use super::{
    LEAD_SIGNATURE, MAX_CLUSTER_FAT12, MAX_CLUSTER_FAT16, STRUCT_SIGNATURE, TRAIL_SIGNATURE,
};

/// BIOS Parameters
#[derive(Debug, Copy, Clone)]
#[repr(packed)]
pub struct BIOSParameterBlock {
    pub(crate) basic_bpb: BasicBPB, // size = 36B
    pub(crate) bpb32: BPB32,        // size = 54B
}

impl BIOSParameterBlock {
    #[inline(always)]
    /// Get the first sector offset bytes of the cluster from the cluster number
    pub fn offset(&self, cluster: u32) -> usize {
        // Q: why cluster - 2?
        // A: The first two clusters are reserved and the first data cluster is 2.
        assert!(cluster >= 2);
        ((self.basic_bpb.rsvd_sec_cnt as usize)
            + (self.basic_bpb.num_fats as usize) * (self.bpb32.fat_sz32 as usize)
            + (cluster as usize - 2) * (self.basic_bpb.sec_per_clus as usize))
            * (self.basic_bpb.byts_per_sec as usize)
    }
    #[inline(always)]
    /// The first data sector beyond the root directory
    ///
    /// The start of the data region, the first sector of cluster 2.
    ///
    // For FAT32, the root directory can be of variable size and is a cluster chain, just like any other
    // directory is. The first cluster of the root directory on a FAT32 volume is stored in BPB_RootClus.
    // Unlike other directories, the root directory itself on any FAT type does not have any date or time
    // stamps, does not have a file name (other than the implied file name “\”), and does not contain “.” and
    // ".." files as the first two directory entries in the directory. The only other special aspect of the root
    // directory is that it is the only directory on the FAT volume for which it is valid to have a file that has
    // only the ATTR_VOLUME_ID attribute bit set.
    ///
    /// The location of the root directory (note that root directory don't have directory entry)
    pub fn first_data_sector(&self) -> usize {
        (self.basic_bpb.rsvd_sec_cnt as usize)
            + (self.basic_bpb.num_fats as usize) * self.bpb32.fat_sz32 as usize
            + self.root_dir_sector_cnt()
    }
    #[inline(always)]
    /// Given any valid data cluster number N, the sector number of the first sector of that cluster
    /// (again relative to sector 0 of the FAT volume) is computed as follows.
    pub fn first_sector_of_cluster(&self, cluster: u32) -> usize {
        self.first_data_sector() + (cluster as usize - 2) * self.basic_bpb.sec_per_clus as usize
    }
    #[inline(always)]
    /// Get FAT1 Offset
    pub fn fat1_offset(&self) -> usize {
        (self.basic_bpb.rsvd_sec_cnt as usize) * (self.basic_bpb.byts_per_sec as usize)
    }
    pub fn fat1_sector_id(&self) -> usize {
        self.basic_bpb.rsvd_sec_cnt as usize
    }
    #[inline(always)]
    /// Get FAT2 Offset
    pub fn fat2_offset(&self) -> usize {
        self.fat1_offset() + (self.bpb32.fat_sz32 as usize) * (self.basic_bpb.byts_per_sec as usize)
    }
    /// Get sector_per_cluster_usize as usize value
    pub fn sector_per_cluster(&self) -> usize {
        self.basic_bpb.sec_per_clus as usize
    }
    #[inline(always)]
    /// Sectors occupied by the root directory
    ///
    /// Note that on a FAT32 volume, the BPB_RootEntCnt value is always 0; so on a FAT32 volume,
    /// RootDirSectors is always 0.
    pub fn root_dir_sector_cnt(&self) -> usize {
        ((self.basic_bpb.root_ent_cnt * 32) as usize + (self.basic_bpb.byts_per_sec - 1) as usize)
            / self.basic_bpb.byts_per_sec as usize
    }
    #[inline(always)]
    /// Total sectors of the data region
    pub fn data_sector_cnt(&self) -> usize {
        self.basic_bpb.tot_sec32 as usize
            - (self.basic_bpb.rsvd_sec_cnt as usize)
            - (self.basic_bpb.num_fats as usize) * (self.bpb32.fat_sz32 as usize)
            - self.root_dir_sector_cnt()
    }
    /// The count of (data) clusters
    ///
    /// This function should round DOWN.
    #[inline(always)]
    pub fn data_cluster_cnt(&self) -> usize {
        self.data_sector_cnt() / (self.basic_bpb.sec_per_clus as usize)
    }
    #[inline(always)]
    /// The total size of the data region
    pub fn total_data_volume(&self) -> usize {
        self.data_sector_cnt() * self.basic_bpb.byts_per_sec as usize
    }
    pub fn is_valid(&self) -> bool {
        self.basic_bpb.root_ent_cnt == 0
            && self.basic_bpb.tot_sec16 == 0
            && self.basic_bpb.tot_sec32 != 0
            && self.basic_bpb.fat_sz16 == 0
            && self.bpb32.fat_sz32 != 0
    }
    #[inline(always)]
    pub fn cluster_size(&self) -> usize {
        self.basic_bpb.sec_per_clus as usize * self.basic_bpb.byts_per_sec as usize
    }
    pub fn fat_type(&self) -> FatType {
        if self.data_cluster_cnt() < MAX_CLUSTER_FAT12 {
            FatType::FAT12
        } else if self.data_cluster_cnt() < MAX_CLUSTER_FAT16 {
            FatType::FAT16
        } else {
            FatType::FAT32
        }
    }
    pub fn bytes_per_sector(&self) -> usize {
        self.basic_bpb.byts_per_sec as usize
    }
    pub fn sectors_per_cluster(&self) -> usize {
        self.basic_bpb.sec_per_clus as usize
    }
    pub fn fat_cnt(&self) -> usize {
        self.basic_bpb.num_fats as usize
    }
    pub fn reserved_sector_cnt(&self) -> usize {
        self.basic_bpb.rsvd_sec_cnt as usize
    }
    pub fn total_sector_cnt(&self) -> usize {
        self.basic_bpb.tot_sec32 as usize
    }
    pub fn sector_pre_fat(&self) -> usize {
        self.bpb32.fat_sz32 as usize
    }
    pub fn root_cluster(&self) -> usize {
        self.bpb32.root_clus as usize
    }
    pub fn fat_info_sector(&self) -> usize {
        self.bpb32.fs_info as usize
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
/// Boot Sector and BPB Structure For FAT12/16/32
pub struct BasicBPB {
    /// x86 assembly to jump instruction to boot code.
    ///
    /// Jump and NOP instructions    Size: 3 bytes    Value: 0xEB ?? 0x90    Offset: 0x00
    pub(crate) _bs_jmp_boot: [u8; 3],
    /// It is only a name string.
    ///
    /// OEM name    Size: 8 bytes    Value: ???    Offset: 0x03
    pub(crate) _bs_oem_name: [u8; 8],
    /// Bytes per sector, This value may take on only the
    /// following values: 512, 1024, 2048 or 4096. 512 for SD card
    ///
    /// Bytes per sector    Size: 2 bytes    Value: 512 (0x200)    Offset: 0x0B
    pub(crate) byts_per_sec: u16,
    /// Sector per cluster. Number of sectors per allocation unit.
    /// Usually 8 for SD card.
    ///
    /// Sector per cluster    Size: 1 byte    Value: 8 (0x08)    Offset: 0x0D
    pub(crate) sec_per_clus: u8,
    /// Sector number of the reserved area.
    ///
    /// Reserved sector count    Size: 2 bytes    Value: 32 (0x20)    Offset: 0x0E
    pub(crate) rsvd_sec_cnt: u16,
    /// This field should always contain the value 2 for any FAT
    /// volume of any type.
    ///
    /// Number of FATs    Size: 1 byte    Value: 2 (0x02)    Offset: 0x10
    pub(crate) num_fats: u8,
    /// For FAT32 volumes, this field must be set to 0.
    pub(crate) root_ent_cnt: u16,
    /// For FAT32 volumes, this field must be 0.
    /// If it is 0, then BPB_TotSec32 must be non-zero.
    ///
    /// Total sectors (for FAT12/16)    Size: 2 bytes    Value: 0 (0x00)    Offset: 0x13
    pub(crate) tot_sec16: u16,
    /// Used to denote the media type.
    ///
    /// Media descriptor    Size: 1 byte    Value: 0xF8 (0xF8)    Offset: 0x15
    pub(crate) _media: u8,
    /// On FAT32 volumes this field must be 0, and fat_sz32 contains the FAT size count.
    ///
    /// FAT size (for FAT12/16)    Size: 2 bytes    Value: 0 (0x00)    Offset: 0x16
    pub(crate) fat_sz16: u16,
    /// Sector per track used by interrupt 0x13.
    /// Not needed by SD card.
    ///
    /// Sectors per track    Size: 2 bytes    Value: 0 (0x00)    Offset: 0x18
    pub(crate) _sec_per_trk: u16,
    /// Number of heads for interrupt 0x13.
    ///
    /// Number of heads    Size: 2 bytes    Value: 0 (0x00)    Offset: 0x1A
    pub(crate) _num_heads: u16,
    /// Count of hidden sectors preceding the partition that contains this FAT volume.
    ///
    /// Hidden sector count    Size: 4 bytes    Value: 0 (0x00)    Offset: 0x1C
    pub(crate) _hidd_sec: u32,
    /// This field is the new 32-bit total count of sectors on the volume.
    ///
    /// Total sectors (for FAT32)    Size: 4 bytes    Value: non-zero    Offset: 0x20
    pub(crate) tot_sec32: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
/// Boot Sector and BPB Structure For FAT32.
/// FAT32 Structure Starting at Offset 36B (0x24B)
pub struct BPB32 {
    /// This field is the FAT32 32-bit count of sectors occupied by
    /// ONE FAT. BPB_FATSz16 must be 0.
    ///
    /// FAT size (for FAT32)    Size: 4 bytes    Value: non-zero    Offset: 0x24
    pub(crate) fat_sz32: u32,
    /// This field is only defined for FAT32 media and does not exist on
    /// FAT12 and FAT16 media.
    /// Bits 0-3    -- Zero-based number of active FAT. Only valid if mirroring
    ///                is disabled.
    /// Bits 4-6    -- Reserved.
    /// Bit 7       -- 0 means the FAT is mirrored at runtime into all FATs.
    ///             -- 1 means only one FAT is active; it is the one referenced
    ///                in bits 0-3.
    /// Bits 8-15   -- Reserved.
    ///
    /// Extended flags    Size: 2 bytes    Value: 0 (0x00)    Offset: 0x28
    pub(crate) _ext_flags: u16,
    /// This is the version number of the FAT32 volume.
    ///
    /// File system version (always 0)    Size: 2 bytes    Value: 0x0000 (0x0000)    Offset: 0x2A
    pub(crate) _fs_ver: u16,
    /// This is set to the cluster number of the first cluster of the root
    /// directory, usually 2 but not required to be 2.
    ///
    /// Root directory first cluster (always 2)    Size: 4 bytes    Value: 2 (0x02)    Offset: 0x2C
    pub(crate) root_clus: u32,
    /// Sector number of FSINFO structure in the reserved area of
    /// the FAT32 volume. Usually 1.
    ///
    /// FSINFO sector (always 1)    Size: 2 bytes    Value: 1 (0x01)    Offset: 0x30
    pub(crate) fs_info: u16,
    /// The sector number in the reserved area of the volume of
    /// a copy of the boot record. Usually 6.
    ///
    /// Backup boot sector (always 6)    Size: 2 bytes    Value: 6 (0x06)    Offset: 0x32
    pub(crate) _bk_boot_sec: u16,
    /// Reserved for future expansion. Code that formats FAT32 volumes
    /// should always set all of the bytes of this field to 0.
    pub(crate) _reserved: [u8; 12],
    /// This field is the physical drive number for the INT 13h.
    ///
    /// Physical drive number    Size: 1 byte    Value: 0x80    Offset: 0x40
    pub(crate) _bs_drv_num: u8,
    /// This field is no longer used and should always be set to 0.
    ///
    /// Reserved (used by Windows NT)    Size: 1 byte    Value: 0 (0x00)    Offset: 0x41
    pub(crate) _bs_reserved1: u8,
    /// This field is the extended boot signature. This field is set to 0x29.
    ///
    /// Extended boot signature    Size: 1 byte    Value: 0x29 (0x29)    Offset: 0x42
    pub(crate) _bs_boot_sig: u8,
    /// Volume serial number.
    ///
    /// Volume serial number    Size: 4 bytes    Value: ???    Offset: 0x43
    pub(crate) _bs_vol_id: u32,
    /// Volume label. This field matches the 11-byte volume label recorded in
    /// the root directory.
    ///
    /// Volume label    Size: 11 bytes    Value: ???    Offset: 0x47
    pub(crate) _bs_vol_lab: [u8; 11],
    /// File system type.
    ///
    /// File system type    Size: 8 bytes    Value: "FAT32   "    Offset: 0x52
    pub(crate) _bs_fil_sys_type: [u8; 8],
}

#[derive(Debug, Clone, Copy)]
#[repr(packed)]
#[allow(dead_code)]
/// Boot Sector and BPB Structure For FAT32.
/// FAT12/16 Structure Starting at Offset 36B (0x24B)
pub struct BPB12_16 {
    bs_drv_num: u8,
    bs_reserved1: u8,
    bs_boot_sig: u8,
    bs_vol_id: u32,
    bs_vol_lab: [u8; 11],
    bs_fil_sys_type: [u8; 8],
}

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
/// FAT32 FSInfo Sector Structure and Backup Boot Sector
pub struct FSInfo {
    /// Value 0x41615252. This lead signature is used to validate that this is in fact an FSInfo sector.
    ///
    /// Lead signature    Size: 4 bytes    Value: 0x41615252    Offset: 0
    pub(crate) lead_sig: u32,
    /// The reserved area should be empty.
    ///
    /// Reserved    Size: 480 bytes    Value: 0    Offset: 4
    pub(crate) _reserved1: [u8; 480],
    /// Value 0x61417272.
    /// Another signature that is more localized in the sector to the location of the fields that are used.
    ///
    /// Structure signature    Size: 4 bytes    Value: 0x61417272    Offset: 484
    pub(crate) struc_sig: u32,
    /// Contains the last known free cluster count on the volume.
    ///
    /// Free cluster count    Size: 4 bytes    Value: 0xFFFFFFFF    Offset: 488
    pub(crate) free_count: u32,
    /// This is a hint for the FAT driver.
    ///
    /// Next free cluster    Size: 4 bytes    Value: 0xFFFFFFFF / ???    Offset: 492
    pub(crate) nxt_free: u32,
    /// The reserved area should be empty.
    ///
    /// Reserved    Size: 12 bytes    Value: 0    Offset: 496
    pub(crate) _reserved2: [u8; 12],
    /// Value 0xAA550000.
    /// This trail signature is used to validate that this is in fact an FSInfo sector.
    ///
    /// Trail signature    Size: 4 bytes    Value: 0xAA550000    Offset: 508
    pub(crate) trail_sig: u32,
}

impl FSInfo {
    // Check the signature
    pub fn check_signature(&self) -> bool {
        self.lead_sig == LEAD_SIGNATURE
            && self.struc_sig == STRUCT_SIGNATURE
            && self.trail_sig == TRAIL_SIGNATURE
    }
    // Get the number of free clusters
    pub fn free_cluster_cnt(&self) -> u32 {
        self.free_count
    }
    // Set the number of free clusters
    pub fn set_free_clusters(&mut self, free_clusters: u32) {
        self.free_count = free_clusters
    }
    // Get next free cluster location
    pub fn next_free_cluster(&self) -> u32 {
        self.nxt_free
    }
    // Set next free cluster location
    pub fn set_next_free_cluster(&mut self, start_cluster: u32) {
        self.nxt_free = start_cluster
    }
}

pub enum FatType {
    FAT32,
    FAT16,
    FAT12,
}
