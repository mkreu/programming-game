use core::{fmt::Write, ptr};

pub struct Log {
    mem: *mut char,
}

impl Log {
    pub const fn bind(slot: usize) -> Self {
        Self {
            mem: slot as *mut char,
        }
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
