mod driving;
mod radar;

pub use driving::Driving;
pub use radar::Radar;

trait Device {
    const MEM_WIDTH: u32;
    fn init_from(mem: *mut u8) -> Self;
}

const IO_BASE_PTR: *mut u8 = 4 as *mut u8;

pub fn get_devices() -> (Radar, Driving) {
    (
        Radar::init_from(IO_BASE_PTR),
        Driving::init_from(IO_BASE_PTR.wrapping_byte_add(Radar::MEM_WIDTH as usize)),
    )
}
