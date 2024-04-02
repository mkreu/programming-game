mod driving;
mod radar;
mod log;

pub use driving::Driving;
pub use radar::Radar;
pub use log::Log;

trait Device {
    const MEM_WIDTH: u32;
    fn init_from(mem: *mut u8) -> Self;
}

const IO_BASE_PTR: *mut u8 = 4 as *mut u8;

pub fn get_devices() -> (Log, Radar, Driving) {
    (   
        Log::init_from(IO_BASE_PTR),
        Radar::init_from(IO_BASE_PTR.wrapping_add(Log::MEM_WIDTH as usize)),
        Driving::init_from(IO_BASE_PTR.wrapping_byte_add(Log::MEM_WIDTH as usize + Radar::MEM_WIDTH as usize)),
    )
}


