use std::{
    thread::{self},
    time::Duration,
};

use tracing::{debug, warn};

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
        cpu.regs[2] = 0x100; // ehh i dont know how stack pointer work...
        return cpu;
    }
    #[allow(dead_code)]
    pub fn fetch(&self) -> u32 {
        //let index = self.pc as usize;
        //return (self.dram.dram[index] as u32)
        //    | ((self.dram.dram[index + 1] as u32) << 8)
        //    | ((self.dram.dram[index + 2] as u32) << 16)
        //    | ((self.dram.dram[index + 3] as u32) << 24);
        return self.dram.load(self.pc, 32).unwrap();
    }
    pub fn execute(&mut self, inst: u32) {
        thread::sleep(Duration::from_millis(200));
        let opcode = inst & 0x7f;
        let funct3 = (inst >> 12) & 0x7;
        let rd = ((inst >> 7) & 0x1f) as usize;
        let rs1 = ((inst >> 15) & 0x1f) as usize;
        let rs2 = ((inst >> 20) & 0x1f) as usize;

        self.regs[0] = 0; // Simulate hard wired x0

        debug!("pc(-4): {:x}, op: {opcode:x}; f3: {funct3:x}", self.pc - 4);

        match opcode {
            0x03 => {
                // load
                // imm[11:0] = inst[31:20]
                let imm = (inst as i32 >> 20) as u32;
                match funct3 {
                    0x0 => {
                        self.regs[rd] = ((self.dram.load(self.regs[rs1] as u32 + imm, 8).unwrap()
                            << 24) as i32
                            >> 24) as u32;
                    }
                    0x1 => {
                        self.regs[rd] = ((self.dram.load(self.regs[rs1] as u32 + imm, 16).unwrap()
                            << 16) as i32
                            >> 16) as u32;
                    }
                    0x2 => {
                        debug!("lw {rd} {imm}({rs1})");
                        self.regs[rd] = self.dram.load(self.regs[rs1] as u32 + imm, 32).unwrap();
                    }
                    0x4 => {
                        self.regs[rd] =
                            self.dram.load(self.regs[rs1] as u32 + imm, 8).unwrap() << 24;
                    }
                    0x5 => {
                        self.regs[rd] =
                            self.dram.load(self.regs[rs1] as u32 + imm, 16).unwrap() << 16;
                    }
                    _ => {
                        panic!("invalid funct3")
                    }
                }
            }
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
            0x23 => {
                // store
                // imm[11:5|4:0] = inst[31:25|11:7]
                let imm = (((inst & 0xfe00_0000) as i32 >> 20) as u32) // imm[11:5]
                    | ((inst >> 7) & 0x1f); // imm[4:0]
                match funct3 {
                    0x0 => {
                        self.dram
                            .store(self.regs[rs1] + imm, 8, self.regs[rs2])
                            .unwrap();
                    }
                    0x1 => {
                        self.dram
                            .store(self.regs[rs1] + imm, 16, self.regs[rs2])
                            .unwrap();
                    }
                    0x2 => {
                        debug!("sw {rs2} {imm}({rs1})");
                        self.dram
                            .store(self.regs[rs1] + imm, 32, self.regs[rs2])
                            .unwrap();
                    }
                    _ => {
                        panic!("invalid funct3")
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
                // auipc
                let imm = inst & 0xfffff000;
                debug!("auipc {rd} {imm}");
                self.regs[rd] = self.pc.wrapping_add(imm).wrapping_sub(4);
            }
            0x63 => {
                // branch
                // imm[12|10:5|4:1|11] = inst[31|30:25|11:8|7]
                let imm = (((inst & 0x8000_0000) as i32 >> 19) as u32) // imm[12]
                    | (inst & 0x7e00_0000) >> 20  // imm[10:5]
                    | ((inst >> 7) & 0x1e) // imm[4:1]
                    | ((inst << 4) & 0x800); // imm[11]
                warn!("{inst:b}");
                warn!("{imm:b}");
                //111111011110
                //000000110000
                debug!("branch");
                match funct3 {
                    0x0 => {
                        if self.regs[rs1] == self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x1 => {
                        if self.regs[rs1] != self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x4 => {
                        if (self.regs[rs1] as i32) < (self.regs[rs2] as i32) {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x5 => {
                        if (self.regs[rs1] as i32) >= (self.regs[rs2] as i32) {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x6 => {
                        if self.regs[rs1] < self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
                        }
                    }
                    0x7 => {
                        if self.regs[rs1] >= self.regs[rs2] {
                            self.pc = self.pc.wrapping_add(imm);
                        }
                    }
                    _ => {
                        panic!("invalid funct3")
                    }
                }
            }
            0x67 => {
                // jalr
                let imm = ((inst & 0xfff0_0000) as i32 >> 20) as u32;
                let immi = imm as i32;
                debug!("jalr {immi}({rs1})");
                let back_addr = self.pc;
                self.pc = self.regs[rs1].wrapping_add(imm) & 0xffff_fffe;
                self.regs[rd] = back_addr
            }
            0x6f => {
                debug!("jal");
                // jal
                self.regs[rd] = self.pc.wrapping_add(4);

                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = (((inst & 0x8000_0000) as i32 >> 11) as u32) // imm[20]
                    | (inst & 0xff000) // imm[19:12]
                    | ((inst >> 9) & 0x800) // imm[11]
                    | ((inst >> 20) & 0x7fe); // imm[10:1]

                self.pc = self.pc.wrapping_add(imm).wrapping_sub(4);
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
