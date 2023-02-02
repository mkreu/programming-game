use log::debug;

use crate::dram::Dram;

pub struct Cpu {
    pub regs: [u32; 32],
    pub pc: u32,
    pub dram: Dram,
}

impl Cpu {
    pub fn new(dram: Dram, entry: u32) -> Self {
        let mut cpu = Self {
            regs: [0; 32],
            pc: entry,
            dram: dram,
        };
        cpu.regs[2] = 128; // ehh i dont know how stack pointer work...
        return cpu;
    }
    #[allow(dead_code)]
    pub fn fetch(&self) -> u32 {
        return self
            .dram
            .load(self.pc, 32)
            .expect("failed to read from memory");
    }
    pub fn execute(&mut self, inst: u32) {
        let opcode = inst & 0x7f;
        let funct3 = (inst >> 12) & 0x7;
        let rd = ((inst >> 7) & 0x1f) as usize;
        let rs1 = ((inst >> 15) & 0x1f) as usize;
        let rs2 = ((inst >> 20) & 0x1f) as usize;

        self.regs[0] = 0; // Simulate hard wired x0

        //println!("opcode: {opcode:b}; {opcode:x}");
        //println!("funct3: {funct3:b}; {funct3:b}");

        match opcode {
            0x13 => {
                let imm = ((inst & 0xfff00000) as i32 >> 20) as u32;
                let shamt = rs2 as u32;
                match funct3 {
                    0x0 => {
                        // addi
                        let immi = imm as i32;
                        debug!("addi {rd} {rs1} {immi}");
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
                        // xori
                        self.regs[rd] = self.regs[rs1] ^ imm;
                    }
                    0x5 => {
                        if (inst & 0x4000_0000) > 0 {
                            // srai
                            self.regs[rd] = ((self.regs[rs1] as i32) >> shamt) as u32;
                        } else {
                            // srli
                            self.regs[rd] = self.regs[rs1] >> shamt;
                        }
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
                match funct3 {
                    0x0 => {
                        if (inst & 0x4000_0000) > 0 {
                            // add
                            self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
                        } else {
                            // sub
                            self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]);
                        }
                    }
                    0x1 => {
                        // sll
                        let shamt = self.regs[rs2] & 0x1f;
                        self.regs[rd] = self.regs[rs1] << shamt;
                    }
                    0x2 => {
                        // slt
                        self.regs[rd] = if (self.regs[rs1] as i32) < (self.regs[rs2] as i32) {
                            1
                        } else {
                            0
                        };
                    }
                    0x3 => {
                        // sltu
                        self.regs[rd] = if self.regs[rs1] < self.regs[rs2] {
                            1
                        } else {
                            0
                        };
                    }
                    0x4 => {
                        // xor
                        self.regs[rd] = self.regs[rs1] ^ self.regs[rs2];
                    }
                    0x5 => {
                        let shamt = self.regs[rs2] & 0x1f;
                        if (inst & 0x4000_0000) > 0 {
                            // sra
                            self.regs[rd] = ((self.regs[rs1] as i32) >> shamt) as u32;
                        } else {
                            // srl
                            self.regs[rd] = self.regs[rs1] >> shamt;
                        }
                    }
                    0x6 => {
                        // or
                        self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                    }
                    0x7 => {
                        // and
                        self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                    }
                    _ => {
                        dbg!("func not implemented yet");
                    }
                }
            }
            0x37 => {
                // lui
                let imm = inst & 0xfffff000;
                let immi = imm >> 12;
                debug!("lui {rd} {rs1} {immi}");
                self.regs[rd] = imm;
            }
            0x17 => {
                // apcui
                let imm = inst & 0xfffff000;
                debug!("apcui {rd} {imm}");
                self.regs[rd] = self.pc.wrapping_add(imm);
            }
            0x6f => {
                panic!("jal");
                // jal
                self.regs[rd] = self.pc.wrapping_add(4);

                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = (((inst & 0x8000_0000) as i32 >> 11) as u32) // imm[20]
                    | (inst & 0xff000) // imm[19:12]
                    | ((inst >> 9) & 0x800) // imm[11]
                    | ((inst >> 20) & 0x7fe); // imm[10:1]

                self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
            }
            0x67 => {
                // jalr
                let imm = ((inst & 0xfff0_0000) as i32 >> 20) as u32;
                debug!("jalr {imm}({rs1})");
                self.regs[rd] = self.pc.wrapping_add(4);
                self.pc = self.regs[rs1].wrapping_add(imm) & 0xffff_fffe
            }
            0x23 => {
                // sw
                let offset = ((inst & 0xfe00_0000) as i32 >> 20) as u32 | (inst & 0x0000_0f80) >> 7;
                let width = match funct3 {
                    0x0 => 8,
                    0x1 => 16,
                    0x2 => 32,
                    _ => {
                        panic!("funct3 {funct3:x} invalid for opcode {opcode:x}");
                    }
                };
                debug!("s({funct3:x}) {rs2} {offset}({rs1})");
                self.dram
                    .store(self.regs[rs1].wrapping_add(offset), width, self.regs[rs2])
                    .expect("failed to write to memory")
            }
            _ => {
                dbg!("opcode not implemented yet");
                panic!()
            }
        }
    }
}

/*#[cfg(test)]
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
        test_itype(0x4, -5, 20, (-5) ^ 20)
    }
    #[test]
    fn test_slli() {
        test_itype(0x1, 4, 13, 4 << 13)
    }
    #[test]
    fn test_srli() {
        test_itype_u(0x5, 0x00130000, 4, 0x00013000);
        test_itype_u(0x5, 0xff130000, 8, 0x00ff1300);
    }
    #[test]
    fn test_srai() {
        let ai = 1 << 10;
        test_itype_u(0x5, 0x00130000, 4 | ai, 0x00013000);
        test_itype_u(0x5, 0xff130000, 8 | ai, 0xffff1300);
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
}*/
