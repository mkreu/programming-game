#[derive(Debug)]
pub enum Instruction {
    R {
        funct: RFunct,
        rd: usize,
        rs1: usize,
        rs2: usize,
    },
    I {
        funct: IFunct,
        rd: usize,
        rs1: usize,
        imm: i32,
    },
    S {
        funct: SFunct,
        rs1: usize,
        rs2: usize,
        imm: i32,
    },
    B {
        funct: BFunct,
        rs1: usize,
        rs2: usize,
        imm: i32,
    },
    U {
        funct: UFunct,
        rd: usize,
        imm: i32,
    },
    J {
        funct: JFunct,
        rd: usize,
        imm: i32,
    },
}

#[derive(Debug)]
pub enum RFunct {
    ADD,
    SUB,
    SLL,
    SLT,
    SLTU,
    XOR,
    SRL,
    SRA,
    OR,
    AND,
}

#[derive(Debug)]
pub enum IFunct {
    JALR,
    LB,
    LH,
    LW,
    LBU,
    LHU,
    ADDI,
    SLTI,
    SLTIU,
    XORI,
    ORI,
    ANDI,
    SLLI,
    SRLI,
    SRAI,
}
#[derive(Debug)]
pub enum SFunct {
    SB,
    SH,
    SW,
}
#[derive(Debug)]
pub enum BFunct {
    BEQ,
    BNE,
    BLT,
    BGE,
    BLTU,
    BGEU,
}

#[derive(Debug)]
pub enum UFunct {
    LUI,
    AUIPC,
}

#[derive(Debug)]
pub enum JFunct {
    JAL,
}

impl Instruction {
    pub fn parse(inst: u32) -> Self {
        let opcode = inst & 0x7f;
        let funct3 = (inst >> 12) & 0x7;
        let rd = ((inst >> 7) & 0x1f) as usize;
        let rs1 = ((inst >> 15) & 0x1f) as usize;
        let rs2 = ((inst >> 20) & 0x1f) as usize;
        match opcode {
            0x03 => {
                use IFunct::*;
                // imm[11:0] = inst[31:20]
                let imm = inst as i32 >> 20;
                let funct = match funct3 {
                    0x0 => LB,
                    0x1 => LH,
                    0x2 => LW,
                    0x4 => LBU,
                    0x5 => LHU,
                    _ => {
                        panic!("invalid funct3")
                    }
                };
                Self::I {
                    funct,
                    rd,
                    rs1,
                    imm,
                }
            }
            0x13 => {
                use IFunct::*;
                let imm = (inst & 0xfff00000) as i32 >> 20;
                let funct = match funct3 {
                    0x0 => ADDI,
                    0x1 => SLLI,
                    0x2 => SLTI,
                    0x3 => SLTIU,
                    0x4 => XORI,
                    0x5 => {
                        if (inst & 0x4000_0000) > 0 {
                            SRAI
                        } else {
                            SRLI
                        }
                    }
                    0x6 => ORI,
                    0x7 => ANDI,
                    _ => {
                        panic!("invalid funct3")
                    }
                };
                match funct {
                    SLLI | SRLI | SRAI => {
                        Self::I {
                            funct,
                            rd,
                            rs1,
                            // rs2 == shamt
                            imm: rs2 as i32,
                        }
                    }
                    _ => Self::I {
                        funct,
                        rd,
                        rs1,
                        imm,
                    },
                }
            }
            0x23 => {
                use SFunct::*;
                // store
                // imm[11:5|4:0] = inst[31:25|11:7]
                let imm = (((inst & 0xfe00_0000) as i32 >> 20) as u32) // imm[11:5]
                    | ((inst >> 7) & 0x1f); // imm[4:0]
                let funct = match funct3 {
                    0x0 => SB,
                    0x1 => SH,
                    0x2 => SW,
                    _ => {
                        panic!("invalid funct3")
                    }
                };
                Self::S {
                    funct,
                    rs1,
                    rs2,
                    imm: imm as i32,
                }
            }
            0x33 => {
                use RFunct::*;
                let funct7 = (inst & 0x4000_0000) > 0;
                let funct = match funct3 {
                    0x0 => {
                        if !funct7 {
                            ADD
                        } else {
                            SUB
                        }
                    }
                    0x1 => SLL,
                    0x2 => SLT,
                    0x3 => SLTU,
                    0x4 => XOR,
                    0x5 => {
                        if !funct7 {
                            SRA
                        } else {
                            SRL
                        }
                    }
                    0x6 => OR,
                    0x7 => AND,
                    _ => {
                        panic!("invalid funct3")
                    }
                };
                Self::R {
                    funct,
                    rd,
                    rs1,
                    rs2,
                }
            }
            0x37 => {
                // lui
                let imm = (inst & 0xfffff000) as i32;
                Self::U {
                    funct: UFunct::LUI,
                    rd,
                    imm,
                }
            }
            0x17 => {
                // auipc
                let imm = (inst & 0xfffff000) as i32;
                Self::U {
                    funct: UFunct::AUIPC,
                    rd,
                    imm,
                }
            }
            0x63 => {
                use BFunct::*;
                // branch
                // imm[12|10:5|4:1|11] = inst[31|30:25|11:8|7]
                let imm = (((inst & 0x8000_0000) as i32 >> 19) as u32) // imm[12]
                    | (inst & 0x7e00_0000) >> 20  // imm[10:5]
                    | ((inst >> 7) & 0x1e) // imm[4:1]
                    | ((inst << 4) & 0x800); // imm[11]
                let imm = imm as i32;
                //111111011110
                //000000110000
                let funct = match funct3 {
                    0x0 => BEQ,
                    0x1 => BNE,
                    0x4 => BLT,
                    0x5 => BGE,
                    0x6 => BLTU,
                    0x7 => BGEU,
                    _ => {
                        panic!("invalid funct3")
                    }
                };
                Self::B {
                    funct,
                    rs1,
                    rs2,
                    imm,
                }
            }
            0x67 => {
                // jalr
                let imm = (inst & 0xfff0_0000) as i32 >> 20;
                Self::I {
                    funct: IFunct::JALR,
                    rd,
                    rs1,
                    imm,
                }
            }
            0x6f => {
                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = (((inst & 0x8000_0000) as i32 >> 11) as u32) // imm[20]
                    | (inst & 0xff000) // imm[19:12]
                    | ((inst >> 9) & 0x800) // imm[11]
                    | ((inst >> 20) & 0x7fe); // imm[10:1]
                let imm = imm as i32;

                Self::J {
                    funct: JFunct::JAL,
                    rd,
                    imm,
                }
            }
            _ => {
                dbg!("opcode not implemented yet");
                panic!()
            }
        }
    }
}
