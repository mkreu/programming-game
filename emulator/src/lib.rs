use cpu::{Hart, Dram};

pub mod cpu;
pub mod bevy;

#[derive(Default)]
pub struct CpuBuilder {}

impl CpuBuilder {
    pub fn build(self, elf: &[u8]) -> (Hart, Dram) {
        let (dram, entry) = Dram::new(elf);
        (Hart::new(entry), dram)
    }
}
