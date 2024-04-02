
use core::{mem::size_of, ptr};

use super::Device;

pub struct Driving {
    mem: *mut u8,
}

impl Device for Driving {
    const MEM_WIDTH: u32 = size_of::<u16>() as u32 * 2;

    fn init_from(mem: *mut u8) -> Self {
        Self { mem }
    }
}

impl Driving {
    pub fn set_speed(&self, speed: u16) {
        unsafe { ptr::write_volatile(self.mem as * mut [u8; 2], speed.to_be_bytes())};
    }
    pub fn set_heading(&self, speed: u16) {
        unsafe { ptr::write_volatile((self.mem as * mut [u8; 2]).wrapping_add(1), speed.to_be_bytes())};
    }
}
