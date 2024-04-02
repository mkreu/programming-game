use core::ptr;

use super::Device;

pub struct Radar {
    mem: *mut u8,
}

impl Device for Radar {
    const MEM_WIDTH: u32 = 16;

    fn init_from(mem: *mut u8) -> Self {
        Self { mem }
    }
}

impl Radar {
    pub const SECTOR_COUNT: u8 = Self::MEM_WIDTH as u8;
    pub fn get_sector_value(&self, sector: u8) -> u8 {
        if (sector as u32) < Self::MEM_WIDTH {
            unsafe { ptr::read_volatile(self.mem.wrapping_add(sector as usize)) }
        } else {
            panic!("invalid sector id")
        }
    }
}
