#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

#[export_name = "main"]
fn main() -> ! {
    let foo = "foobar";
    let a = 1;
    let b = 2;
    let c = a + b;
    loop {}
}
