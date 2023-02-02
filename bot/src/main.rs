#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}

#[export_name = "main"]
fn main() -> ! {
    let a = 1;
    let b = 2;
    let _c = a + b;
    let _foo = "foobar";
    panic!();
    //loop {}
}
