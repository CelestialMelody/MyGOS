//! Signature Fields of Virtual Address Space Mapping

use nix::MmapProts;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct MapPermission: u64 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

#[allow(unused)]
impl MapPermission {
    pub fn from_vm_prot(prot: MmapProts) -> Self {
        if prot.bits() == 0 {
            return MapPermission::empty();
        }

        macro_rules! prot2flags {
            ($flags:expr, $($prot_bit:expr, $flag_bit:expr)*) => {
                $(
                    if prot.contains($prot_bit) {
                        $flags |= $flag_bit;
                    }
                )*
            };
        }

        let mut flags = MapPermission::empty();

        prot2flags! {
            flags,
            MmapProts::PROT_READ,  MapPermission::R
            MmapProts::PROT_WRITE, MapPermission::W
            MmapProts::PROT_EXEC,  MapPermission::X
        }

        flags
    }

    pub fn readable(self) -> bool {
        self.contains(MapPermission::R)
    }

    pub fn writable(self) -> bool {
        self.contains(MapPermission::W)
    }

    pub fn executable(self) -> bool {
        self.contains(MapPermission::X)
    }

    pub fn is_user(self) -> bool {
        self.contains(MapPermission::U)
    }
}
