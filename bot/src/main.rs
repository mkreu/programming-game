#![no_std]
#![no_main]

use core::{fmt::Write, panic::PanicInfo};

use devices::Radar;

mod devices;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    let (mut log, _, _) = devices::get_devices();
    writeln!(log, "{}", _panic).ok(); // Do not panic in panic
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
    let (mut log, radar, driving) = devices::get_devices();
    driving.set_speed(10);
    loop {
        let best_sector = (0..Radar::SECTOR_COUNT)
            .max_by_key(|sector| radar.get_sector_value(*sector))
            .unwrap();
        writeln!(&mut log, "Best Sector: {}", best_sector).unwrap();
        driving.set_heading((best_sector as u16 * 2 + 1) * 180 / Radar::SECTOR_COUNT as u16)
    }
}

