use core::{fmt::Write, mem::size_of, ptr};

use super::Device;

pub struct Log {
    mem: *mut char,
}

impl Device for Log {
    const MEM_WIDTH: u32 = size_of::<char>() as u32;

    fn init_from(mem: *mut u8) -> Self {
        Self {
            mem: mem as *mut char,
        }
    }
}

impl Log {
    pub fn log_char(&self, c: char) {
        unsafe { ptr::write_volatile(self.mem, c) };
    }

    pub fn log_line(&self, s: &'_ str) {
        s.chars().for_each(|c| unsafe {
            ptr::write_volatile(self.mem, c);
        });
        unsafe {
            ptr::write_volatile(self.mem, '\n');
        };
    }
}
impl Write for Log {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        s.chars().for_each(|c| unsafe {
            ptr::write_volatile(self.mem, c);
        });
        Ok(())
    }
}
