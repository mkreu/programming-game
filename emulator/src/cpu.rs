use crate::dram::{Dram, DRAM_SIZE};

use self::instruction::Instruction;

pub mod instruction;

#[derive(Debug)]
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
            dram,
        };
        cpu.regs[2] = DRAM_SIZE - 8;
        cpu
    }
    #[allow(dead_code)]
    pub fn fetch(&self) -> u32 {
        self.dram.load(self.pc, 32).unwrap()
    }
    pub fn execute(&mut self, inst: Instruction) {
        self.regs[0] = 0; // Simulate hard wired x0
        match inst {
            Instruction::R {
                funct,
                rd,
                rs1,
                rs2,
            } => match funct {
                instruction::RFunct::ADD => {
                    self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
                }
                instruction::RFunct::SUB => {
                    self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]);
                }
                instruction::RFunct::SLL => {
                    let shamt = self.regs[rs2] & 0x1f;
                    self.regs[rd] = self.regs[rs1] << shamt;
                }
                instruction::RFunct::SLT => {
                    self.regs[rd] = if (self.regs[rs1] as i32) < (self.regs[rs2] as i32) {
                        1
                    } else {
                        0
                    };
                }
                instruction::RFunct::SLTU => {
                    self.regs[rd] = if self.regs[rs1] < self.regs[rs2] {
                        1
                    } else {
                        0
                    };
                }
                instruction::RFunct::XOR => {
                    self.regs[rd] = self.regs[rs1] ^ self.regs[rs2];
                }
                instruction::RFunct::SRL => {
                    let shamt = self.regs[rs2] & 0x1f;
                    self.regs[rd] = self.regs[rs1] >> shamt;
                }
                instruction::RFunct::SRA => {
                    let shamt = self.regs[rs2] & 0x1f;
                    self.regs[rd] = self.regs[rs1] >> shamt;
                }
                instruction::RFunct::OR => {
                    self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                }
                instruction::RFunct::AND => {
                    self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                }
            },
            Instruction::I {
                funct,
                rd,
                rs1,
                imm,
            } => match funct {
                instruction::IFunct::JALR => {
                    let back_addr = self.pc;
                    self.pc = self.regs[rs1].wrapping_add_signed(imm) & 0xffff_fffe;
                    self.regs[rd] = back_addr
                }
                instruction::IFunct::LB => {
                    self.regs[rd] = ((self
                        .dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 8)
                        .unwrap()
                        << 24) as i32
                        >> 24) as u32;
                }
                instruction::IFunct::LH => {
                    self.regs[rd] = ((self
                        .dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 16)
                        .unwrap()
                        << 16) as i32
                        >> 16) as u32;
                }
                instruction::IFunct::LW => {
                    self.regs[rd] = self
                        .dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 32)
                        .unwrap();
                }
                instruction::IFunct::LBU => {
                    self.regs[rd] = self
                        .dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 8)
                        .unwrap();
                }
                instruction::IFunct::LHU => {
                    self.regs[rd] = self
                        .dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 16)
                        .unwrap();
                }
                instruction::IFunct::ADDI => {
                    self.regs[rd] = self.regs[rs1].wrapping_add_signed(imm);
                }
                instruction::IFunct::SLTI => {
                    self.regs[rd] = if (self.regs[rs1] as i32) < imm { 1 } else { 0 };
                }
                instruction::IFunct::SLTIU => {
                    self.regs[rd] = if self.regs[rs1] < (imm as u32) { 1 } else { 0 };
                }
                instruction::IFunct::XORI => {
                    self.regs[rd] = self.regs[rs1] ^ (imm as u32);
                }
                instruction::IFunct::ORI => {
                    self.regs[rd] = self.regs[rs1] | (imm as u32);
                }
                instruction::IFunct::ANDI => {
                    self.regs[rd] = self.regs[rs1] & (imm as u32);
                }
                instruction::IFunct::SLLI => {
                    self.regs[rd] = self.regs[rs1] << imm;
                }
                instruction::IFunct::SRLI => {
                    self.regs[rd] = self.regs[rs1] >> imm;
                }
                instruction::IFunct::SRAI => {
                    self.regs[rd] = ((self.regs[rs1] as i32) >> imm) as u32;
                }
            },
            Instruction::S {
                funct,
                rs1,
                rs2,
                imm,
            } => match funct {
                instruction::SFunct::SB => {
                    self.dram
                        .store(self.regs[rs1].wrapping_add_signed(imm), 8, self.regs[rs2])
                        .unwrap();
                }
                instruction::SFunct::SH => {
                    self.dram
                        .store(self.regs[rs1].wrapping_add_signed(imm), 16, self.regs[rs2])
                        .unwrap();
                }
                instruction::SFunct::SW => {
                    self.dram
                        .store(self.regs[rs1].wrapping_add_signed(imm), 32, self.regs[rs2])
                        .unwrap();
                }
            },
            Instruction::B {
                funct,
                rs1,
                rs2,
                imm,
            } => match funct {
                instruction::BFunct::BEQ => {
                    if self.regs[rs1] == self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                    }
                }
                instruction::BFunct::BNE => {
                    if self.regs[rs1] != self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                    }
                }
                instruction::BFunct::BLT => {
                    if (self.regs[rs1] as i32) < (self.regs[rs2] as i32) {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                    }
                }
                instruction::BFunct::BGE => {
                    if (self.regs[rs1] as i32) >= (self.regs[rs2] as i32) {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                    }
                }
                instruction::BFunct::BLTU => {
                    if self.regs[rs1] < self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                    }
                }
                instruction::BFunct::BGEU => {
                    if self.regs[rs1] >= self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                    }
                }
            },
            Instruction::U { funct, rd, imm } => match funct {
                instruction::UFunct::LUI => {
                    self.regs[rd] = imm as u32;
                }
                instruction::UFunct::AUIPC => {
                    self.regs[rd] = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                }
            },
            Instruction::J { funct, rd, imm } => match funct {
                instruction::JFunct::JAL => {
                    self.regs[rd] = self.pc.wrapping_add(4);
                    self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(4);
                }
            },
        }
    }
}
