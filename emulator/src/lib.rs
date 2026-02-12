use cpu::{Dram, Hart};

pub mod bevy;
pub mod cpu;

#[derive(Default)]
pub struct CpuBuilder {}

impl CpuBuilder {
    pub fn build(self, elf: &[u8]) -> (Hart, Dram) {
        let (dram, entry) = Dram::new(elf);
        let mut hart = Hart::new(entry);
        let stack_top = (dram.dram.len() as u32) & !0xf;
        hart.regs[2] = stack_top.saturating_sub(16);
        (hart, dram)
    }
}
