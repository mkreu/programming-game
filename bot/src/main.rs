#![no_std]
#![no_main]

use core::panic::PanicInfo;

use devices::Radar;

mod devices;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

pub const DRAM_SIZE: u32 = 1024 * 64;

pub enum Direction {
    NONE,
    LEFT,
    UP,
    RIGHT,
    DOWN,
}

#[export_name = "main"]
fn main() -> ! {
    let (radar, driving) = devices::get_devices();
    driving.set_speed(10);
    loop {
        let best_sector = (0..Radar::SECTOR_COUNT).max_by_key(|sector| radar.get_sector_value(*sector)).unwrap();
        driving.set_heading(best_sector as u16 * 360 / Radar::SECTOR_COUNT as u16)
    }
}
