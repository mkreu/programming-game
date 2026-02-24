#![no_std]
#![no_main]

use core::fmt::Write;

use botracers_bot_sdk::{SLOT3, driving::CarControls, log};

#[unsafe(export_name = "main")]
fn main() -> ! {
    writeln!(log(), "Car Radar OS starting up...").ok();

    let mut car_controls = CarControls::bind(SLOT3);

    car_controls.set_accelerator(1.0);

    loop {}
}
