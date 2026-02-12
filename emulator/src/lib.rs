use cpu::{Dram, Hart};

pub mod bevy;
pub mod cpu;

#[derive(Default)]
pub struct CpuBuilder {}

fn stack_pointer_for_dram_len(dram_len: u32) -> u32 {
    let stack_top = dram_len & !0xf;
    stack_top.saturating_sub(16)
}

impl CpuBuilder {
    pub fn build(self, elf: &[u8]) -> (Hart, Dram) {
        let (dram, entry) = Dram::new(elf);
        let mut hart = Hart::new(entry);
        hart.regs[2] = stack_pointer_for_dram_len(dram.dram.len() as u32);
        (hart, dram)
    }
}

#[cfg(test)]
mod tests {
    use super::stack_pointer_for_dram_len;

    #[test]
    fn stack_pointer_for_dram_len_is_16_byte_aligned() {
        assert_eq!(stack_pointer_for_dram_len(0x42562), 0x42550);
        assert_eq!(stack_pointer_for_dram_len(0x42560), 0x42550);
        assert_eq!(stack_pointer_for_dram_len(15), 0);
    }
}
