use cpu::Cpu;
use dram::Dram;

pub mod cpu;
pub mod dram;
pub mod io;

#[derive(Default)]
pub struct CpuBuilder {}

impl CpuBuilder {
    pub fn build(self, elf: &[u8]) -> Cpu {
        let (dram, entry) = Dram::new(elf);
        Cpu::new(dram, entry)
    }
}

