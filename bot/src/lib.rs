#![no_std]

use crate::log::Log;

pub mod log;
pub mod driving;

pub const SLOT1: usize = 0x100;
pub const SLOT2: usize = 0x200;
pub const SLOT3: usize = 0x300;
pub const SLOT4: usize = 0x400;

pub fn log() -> Log {
    Log::bind(SLOT1)
}
