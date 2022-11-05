pub struct Cpu {
    pub regs: [u32; 32],
    pub pc: u32,
    pub dram: Vec<u8>,
}

impl Cpu {
    pub fn new(code: Vec<u8>) -> Self {
        Self {
            regs: [0; 32],
            pc: 0,
            dram: code,
        }
    }
    #[allow(dead_code)]
    pub fn fetch(&self) -> u32 {
        let index = self.pc as usize;
        return (self.dram[index] as u32)
            | ((self.dram[index + 1] as u32) << 8)
            | ((self.dram[index + 2] as u32) << 16)
            | ((self.dram[index + 3] as u32) << 24);
    }
    pub fn execute(&mut self, inst: u32) {
        let opcode = inst & 0x7f;
        let funct3 = (inst >> 12) & 0x7;
        let rd = ((inst >> 7) & 0x1f) as usize;
        let rs1 = ((inst >> 15) & 0x1f) as usize;
        let rs2 = ((inst >> 20) & 0x1f) as usize;

        self.regs[0] = 0; // Simulate hard wired x0

        println!("opcode: {opcode:x}");
        println!("funct3: {funct3:x}");

        match opcode {
            0x13 => {
                let imm = ((inst & 0xfff00000) as i32 >> 20) as u32;
                let shamt = ((inst & 0x003f00000) as i32 >> 20) as u32;
                match funct3 {
                    0x0 => {
                        // addi
                        self.regs[rd] = self.regs[rs1].wrapping_add(imm);
                    }
                    0x1 => {
                        // slli
                        self.regs[rd] = self.regs[rs1] << shamt;
                    }
                    0x2 => {
                        // stli
                        self.regs[rd] = if (self.regs[rs1] as i32) < (imm as i32) {
                            1
                        } else {
                            0
                        };
                    }
                    0x3 => {
                        // stliu
                        self.regs[rd] = if self.regs[rs1] < imm { 1 } else { 0 };
                    }
                    0x4 => {
                        if (inst & 0x4000_0000) > 0 {
                            // srai
                            self.regs[rd] = ((self.regs[rs1] as i32) >> shamt) as u32;
                        } else {
                            // srli
                            self.regs[rd] = self.regs[rs1] >> shamt;
                        }
                    }
                    0x5 => {
                        // xori
                        self.regs[rd] = self.regs[rs1] ^ imm;
                    }
                    0x6 => {
                        // ori
                        self.regs[rd] = self.regs[rs1] | imm;
                    }
                    0x7 => {
                        // andi
                        self.regs[rd] = self.regs[rs1] & imm;
                    }
                    _ => {
                        dbg!("func not implemented yet");
                    }
                }
            }
            0x33 => {
                // add
                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
            }
            _ => {
                dbg!("opcode not implemented yet");
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use super::Cpu;

    #[test]
    fn test_addi() {
        test_itype(0, 21, -42, -21);
        test_itype(0, i32::MAX, 1, i32::MIN);
        test_itype_u(0, 0, 18, 18);
    }
    #[test]
    fn test_stli() {
        test_itype(2, -5, 20, 1);
    }
    #[test]
    fn test_stliu() {
        test_itype(3, -5, 20, 0);
    }
    #[test]
    fn test_andi() {
        test_itype(0x7, -5, 20, (-5) & 20);
        test_itype(0x7, -20, 5, (-20) & 5);
    }
    #[test]
    fn test_ori() {
        test_itype(0x6, -5, 20, (-5) | 20)
    }
    #[test]
    fn test_xori() {
        test_itype(0x5, -5, 20, (-5) ^ 20)
    }
    #[test]
    fn test_slli() {
        test_itype(0x1, 4, 13, 4 << 13)
    }
    #[test]
    fn test_srli() {
        test_itype_u(0x4, 0x00130000, 4, 0x00013000);
        test_itype_u(0x4, 0xff130000, 8, 0x00ff1300);
    }
    #[test]
    fn test_srai() {
        let ai = 1 << 10;
        test_itype_u(0x4, 0x00130000, 4 | ai, 0x00013000);
        test_itype_u(0x4, 0xff130000, 8 | ai, 0xffff1300);
    }

    fn test_itype_u(funct3: u32, reg_val: u32, imm: i32, uut: u32) {
        let mut cpu = Cpu::new(vec![]);
        let inst = IType {
            opcode: 0x13,
            funct3,
            rd: 1,
            rs1: 3,
            imm,
        }
        .build();

        cpu.regs[3] = reg_val;
        cpu.execute(inst);

        assert_eq!(
            uut, cpu.regs[1],
            "expected {}, but was {}",
            uut, cpu.regs[1]
        )
    }
    fn test_itype(funct3: u32, reg_val: i32, imm: i32, uut: i32) {
        let mut cpu = Cpu::new(vec![]);
        let inst = IType {
            opcode: 0x13,
            funct3,
            rd: 1,
            rs1: 3,
            imm,
        }
        .build();

        cpu.regs[3] = reg_val as u32;
        cpu.execute(inst);

        assert_eq!(
            uut, cpu.regs[1] as i32,
            "expected {}, but was {}",
            uut, cpu.regs[1]
        )
    }
    trait Instruction {
        fn build(&self) -> u32;
    }

    struct IType {
        opcode: u32,
        funct3: u32,
        rd: u32,
        rs1: u32,
        imm: i32,
    }

    impl Instruction for IType {
        fn build(&self) -> u32 {
            self.opcode
                | self.rd << 7
                | self.funct3 << 12
                | self.rs1 << 15
                | (self.imm as u32) << 20
        }
    }
}
