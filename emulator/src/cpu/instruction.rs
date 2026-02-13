#[derive(Debug)]
pub enum Instruction {
    R {
        funct: RFunct,
        rd: usize,
        rs1: usize,
        rs2: usize,
    },
    M {
        funct: MFunct,
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
    R4 {
        funct: R4Funct,
        rd: usize,
        rs1: usize,
        rs2: usize,
        rs3: usize,
        rm: u32,
    },
    FR {
        funct: FRFunct,
        rd: usize,
        rs1: usize,
        rs2: usize,
        rm: u32,
    },
    FI {
        funct: FIFunct,
        rd: usize,
        rs1: usize,
        rm: u32,
    },
    FL {
        funct: FLFunct,
        rd: usize,
        rs1: usize,
        imm: i32,
    },
    FS {
        funct: FSFunct,
        rs1: usize,
        rs2: usize,
        imm: i32,
    },
    A {
        funct: AFunct,
        rd: usize,
        rs1: usize,
        rs2: usize,
        aq: bool,
        rl: bool,
    },
    Fence {
        funct: FenceFunct,
        pred: u32,
        succ: u32,
        fm: u32,
    },
    Ebreak,
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
pub enum MFunct {
    MUL,
    MULH,
    MULHSU,
    MULHU,
    DIV,
    DIVU,
    REM,
    REMU,
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

#[derive(Debug)]
pub enum R4Funct {
    FmaddS,
    FmsubS,
    FnmsubS,
    FnmaddS,
}

#[derive(Debug)]
pub enum FRFunct {
    FaddS,
    FsubS,
    FmulS,
    FdivS,
    FsgnjS,
    FsgnjnS,
    FsgnjxS,
    FminS,
    FmaxS,
    FeqS,
    FltS,
    FleS,
}

#[derive(Debug)]
pub enum FIFunct {
    FsqrtS,
    FcvtWS,
    FcvtWuS,
    FmvXW,
    FclassS,
    FcvtSW,
    FcvtSWU,
    FmvWX,
}

#[derive(Debug)]
pub enum FLFunct {
    FLH,
    FLW,
    FLD,
}

#[derive(Debug)]
pub enum FSFunct {
    FSH,
    FSW,
    FSD,
}

#[derive(Debug)]
pub enum AFunct {
    LrW,
    ScW,
    AmoSwapW,
    AmoAddW,
    AmoXorW,
    AmoAndW,
    AmoOrW,
    AmoMinW,
    AmoMaxW,
    AmoMinuW,
    AmoMaxuW,
}

#[derive(Debug)]
pub enum FenceFunct {
    Fence,
    FenceI,
}

fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

fn cbit(inst: u32, bit: u32) -> u32 {
    (inst >> bit) & 1
}

impl Instruction {
    pub fn parse(inst: u32) -> Self {
        Self::parse_with_len(inst).0
    }

    pub fn parse_with_len(inst: u32) -> (Self, u32) {
        if (inst & 0x3) != 0x3 {
            (Self::parse_compressed(inst & 0xffff), 2)
        } else {
            (Self::parse_32(inst), 4)
        }
    }

    fn parse_32(inst: u32) -> Self {
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
                    _ => panic!("invalid funct3"),
                };
                Self::I {
                    funct,
                    rd,
                    rs1,
                    imm,
                }
            }
            0x0f => {
                let funct = match funct3 {
                    0x0 => FenceFunct::Fence,
                    0x1 => FenceFunct::FenceI,
                    _ => panic!("invalid funct3"),
                };
                let pred = (inst >> 24) & 0xf;
                let succ = (inst >> 20) & 0xf;
                let fm = (inst >> 28) & 0xf;
                Self::Fence {
                    funct,
                    pred,
                    succ,
                    fm,
                }
            }
            0x13 => {
                use IFunct::*;
                let imm = (inst & 0xfff0_0000) as i32 >> 20;
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
                    _ => panic!("invalid funct3"),
                };
                match funct {
                    SLLI | SRLI | SRAI => Self::I {
                        funct,
                        rd,
                        rs1,
                        imm: rs2 as i32,
                    },
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
                // imm[11:5|4:0] = inst[31:25|11:7]
                let imm_u = ((inst >> 20) & 0xfe0) | ((inst >> 7) & 0x1f);
                let funct = match funct3 {
                    0x0 => SB,
                    0x1 => SH,
                    0x2 => SW,
                    _ => panic!("invalid funct3"),
                };
                Self::S {
                    funct,
                    rs1,
                    rs2,
                    imm: sign_extend(imm_u, 12),
                }
            }
            0x2f => {
                if funct3 != 0x2 {
                    panic!("invalid atomic funct3");
                }
                let funct5 = (inst >> 27) & 0x1f;
                let aq = cbit(inst, 26) == 1;
                let rl = cbit(inst, 25) == 1;
                let funct = match funct5 {
                    0x02 => AFunct::LrW,
                    0x03 => AFunct::ScW,
                    0x01 => AFunct::AmoSwapW,
                    0x00 => AFunct::AmoAddW,
                    0x04 => AFunct::AmoXorW,
                    0x0c => AFunct::AmoAndW,
                    0x08 => AFunct::AmoOrW,
                    0x10 => AFunct::AmoMinW,
                    0x14 => AFunct::AmoMaxW,
                    0x18 => AFunct::AmoMinuW,
                    0x1c => AFunct::AmoMaxuW,
                    _ => panic!("invalid atomic funct5"),
                };
                if matches!(funct, AFunct::LrW) && rs2 != 0 {
                    panic!("invalid LR.W encoding");
                }
                Self::A {
                    funct,
                    rd,
                    rs1,
                    rs2,
                    aq,
                    rl,
                }
            }
            0x33 => {
                let funct7 = (inst >> 25) & 0x7f;
                if funct7 == 0x01 {
                    use MFunct::*;
                    let funct = match funct3 {
                        0x0 => MUL,
                        0x1 => MULH,
                        0x2 => MULHSU,
                        0x3 => MULHU,
                        0x4 => DIV,
                        0x5 => DIVU,
                        0x6 => REM,
                        0x7 => REMU,
                        _ => panic!("invalid M funct3"),
                    };
                    return Self::M {
                        funct,
                        rd,
                        rs1,
                        rs2,
                    };
                }

                use RFunct::*;
                let funct = match funct3 {
                    0x0 => {
                        if funct7 == 0 {
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
                        if funct7 == 0 {
                            SRL
                        } else {
                            SRA
                        }
                    }
                    0x6 => OR,
                    0x7 => AND,
                    _ => panic!("invalid funct3"),
                };
                Self::R {
                    funct,
                    rd,
                    rs1,
                    rs2,
                }
            }
            0x37 => {
                let imm = (inst & 0xfffff000) as i32;
                Self::U {
                    funct: UFunct::LUI,
                    rd,
                    imm,
                }
            }
            0x17 => {
                let imm = (inst & 0xfffff000) as i32;
                Self::U {
                    funct: UFunct::AUIPC,
                    rd,
                    imm,
                }
            }
            0x63 => {
                use BFunct::*;
                // imm[12|10:5|4:1|11] = inst[31|30:25|11:8|7]
                let imm_u = ((inst >> 19) & 0x1000)
                    | ((inst >> 20) & 0x7e0)
                    | ((inst >> 7) & 0x1e)
                    | ((inst << 4) & 0x800);
                let funct = match funct3 {
                    0x0 => BEQ,
                    0x1 => BNE,
                    0x4 => BLT,
                    0x5 => BGE,
                    0x6 => BLTU,
                    0x7 => BGEU,
                    _ => panic!("invalid funct3"),
                };
                Self::B {
                    funct,
                    rs1,
                    rs2,
                    imm: sign_extend(imm_u, 13),
                }
            }
            0x67 => {
                let imm = (inst & 0xfff0_0000) as i32 >> 20;
                Self::I {
                    funct: IFunct::JALR,
                    rd,
                    rs1,
                    imm,
                }
            }
            0x07 => {
                let imm = inst as i32 >> 20;
                let funct = match funct3 {
                    0x1 => FLFunct::FLH,
                    0x2 => FLFunct::FLW,
                    0x3 => FLFunct::FLD,
                    _ => panic!("invalid funct3"),
                };
                Self::FL {
                    funct,
                    rd,
                    rs1,
                    imm,
                }
            }
            0x27 => {
                let imm_u = ((inst >> 20) & 0xfe0) | ((inst >> 7) & 0x1f);
                let funct = match funct3 {
                    0x1 => FSFunct::FSH,
                    0x2 => FSFunct::FSW,
                    0x3 => FSFunct::FSD,
                    _ => panic!("invalid funct3"),
                };
                Self::FS {
                    funct,
                    rs1,
                    rs2,
                    imm: sign_extend(imm_u, 12),
                }
            }
            0x43 | 0x47 | 0x4b | 0x4f => {
                let rs3 = ((inst >> 27) & 0x1f) as usize;
                let fmt = (inst >> 25) & 0x3;
                if fmt != 0 {
                    panic!("only single-precision format is supported");
                }
                let rm = funct3;
                let funct = match opcode {
                    0x43 => R4Funct::FmaddS,
                    0x47 => R4Funct::FmsubS,
                    0x4b => R4Funct::FnmsubS,
                    0x4f => R4Funct::FnmaddS,
                    _ => unreachable!(),
                };
                Self::R4 {
                    funct,
                    rd,
                    rs1,
                    rs2,
                    rs3,
                    rm,
                }
            }
            0x53 => {
                let funct7 = (inst >> 25) & 0x7f;
                let rm = funct3;
                match funct7 {
                    0x00 => Self::FR {
                        funct: FRFunct::FaddS,
                        rd,
                        rs1,
                        rs2,
                        rm,
                    },
                    0x04 => Self::FR {
                        funct: FRFunct::FsubS,
                        rd,
                        rs1,
                        rs2,
                        rm,
                    },
                    0x08 => Self::FR {
                        funct: FRFunct::FmulS,
                        rd,
                        rs1,
                        rs2,
                        rm,
                    },
                    0x0c => Self::FR {
                        funct: FRFunct::FdivS,
                        rd,
                        rs1,
                        rs2,
                        rm,
                    },
                    0x10 => {
                        let funct = match rm {
                            0x0 => FRFunct::FsgnjS,
                            0x1 => FRFunct::FsgnjnS,
                            0x2 => FRFunct::FsgnjxS,
                            _ => panic!("invalid funct3"),
                        };
                        Self::FR {
                            funct,
                            rd,
                            rs1,
                            rs2,
                            rm,
                        }
                    }
                    0x14 => {
                        let funct = match rm {
                            0x0 => FRFunct::FminS,
                            0x1 => FRFunct::FmaxS,
                            _ => panic!("invalid funct3"),
                        };
                        Self::FR {
                            funct,
                            rd,
                            rs1,
                            rs2,
                            rm,
                        }
                    }
                    0x2c => Self::FI {
                        funct: FIFunct::FsqrtS,
                        rd,
                        rs1,
                        rm,
                    },
                    0x50 => {
                        let funct = match rm {
                            0x2 => FRFunct::FeqS,
                            0x1 => FRFunct::FltS,
                            0x0 => FRFunct::FleS,
                            _ => panic!("invalid funct3"),
                        };
                        Self::FR {
                            funct,
                            rd,
                            rs1,
                            rs2,
                            rm,
                        }
                    }
                    0x60 => {
                        let funct = match rs2 {
                            0x0 => FIFunct::FcvtWS,
                            0x1 => FIFunct::FcvtWuS,
                            _ => panic!("invalid rs2"),
                        };
                        Self::FI { funct, rd, rs1, rm }
                    }
                    0x68 => {
                        let funct = match rs2 {
                            0x0 => FIFunct::FcvtSW,
                            0x1 => FIFunct::FcvtSWU,
                            _ => panic!("invalid rs2"),
                        };
                        Self::FI { funct, rd, rs1, rm }
                    }
                    0x70 => {
                        let funct = match rm {
                            0x0 => FIFunct::FmvXW,
                            0x1 => FIFunct::FclassS,
                            _ => panic!("invalid funct3"),
                        };
                        Self::FI { funct, rd, rs1, rm }
                    }
                    0x78 => Self::FI {
                        funct: FIFunct::FmvWX,
                        rd,
                        rs1,
                        rm,
                    },
                    _ => panic!("invalid funct7"),
                }
            }
            0x6f => {
                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm_u = ((inst >> 11) & 0x100000)
                    | (inst & 0xff000)
                    | ((inst >> 9) & 0x800)
                    | ((inst >> 20) & 0x7fe);

                Self::J {
                    funct: JFunct::JAL,
                    rd,
                    imm: sign_extend(imm_u, 21),
                }
            }
            _ => {
                dbg!("opcode not implemented yet", opcode);
                panic!()
            }
        }
    }

    fn parse_compressed(inst: u32) -> Self {
        let quadrant = inst & 0x3;
        let funct3 = (inst >> 13) & 0x7;

        match quadrant {
            0b00 => match funct3 {
                0b000 => {
                    // C.ADDI4SPN
                    let nzuimm = ((inst >> 6) & 0x1) << 2
                        | ((inst >> 5) & 0x1) << 3
                        | ((inst >> 11) & 0x3) << 4
                        | ((inst >> 7) & 0xf) << 6;
                    if nzuimm == 0 {
                        panic!("illegal C.ADDI4SPN");
                    }
                    let rd = 8 + ((inst >> 2) & 0x7) as usize;
                    Self::I {
                        funct: IFunct::ADDI,
                        rd,
                        rs1: 2,
                        imm: nzuimm as i32,
                    }
                }
                0b010 => {
                    // C.LW
                    let uimm = ((inst >> 6) & 0x1) << 2
                        | ((inst >> 10) & 0x7) << 3
                        | ((inst >> 5) & 0x1) << 6;
                    let rd = 8 + ((inst >> 2) & 0x7) as usize;
                    let rs1 = 8 + ((inst >> 7) & 0x7) as usize;
                    Self::I {
                        funct: IFunct::LW,
                        rd,
                        rs1,
                        imm: uimm as i32,
                    }
                }
                0b011 => {
                    // C.FLW (RV32)
                    let uimm = ((inst >> 6) & 0x1) << 2
                        | ((inst >> 10) & 0x7) << 3
                        | ((inst >> 5) & 0x1) << 6;
                    let rd = 8 + ((inst >> 2) & 0x7) as usize;
                    let rs1 = 8 + ((inst >> 7) & 0x7) as usize;
                    Self::FL {
                        funct: FLFunct::FLW,
                        rd,
                        rs1,
                        imm: uimm as i32,
                    }
                }
                0b110 => {
                    // C.SW
                    let uimm = ((inst >> 6) & 0x1) << 2
                        | ((inst >> 10) & 0x7) << 3
                        | ((inst >> 5) & 0x1) << 6;
                    let rs1 = 8 + ((inst >> 7) & 0x7) as usize;
                    let rs2 = 8 + ((inst >> 2) & 0x7) as usize;
                    Self::S {
                        funct: SFunct::SW,
                        rs1,
                        rs2,
                        imm: uimm as i32,
                    }
                }
                0b111 => {
                    // C.FSW (RV32)
                    let uimm = ((inst >> 6) & 0x1) << 2
                        | ((inst >> 10) & 0x7) << 3
                        | ((inst >> 5) & 0x1) << 6;
                    let rs1 = 8 + ((inst >> 7) & 0x7) as usize;
                    let rs2 = 8 + ((inst >> 2) & 0x7) as usize;
                    Self::FS {
                        funct: FSFunct::FSW,
                        rs1,
                        rs2,
                        imm: uimm as i32,
                    }
                }
                _ => panic!("illegal compressed instruction (quadrant 0)"),
            },
            0b01 => match funct3 {
                0b000 => {
                    // C.NOP/C.ADDI
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    let imm = sign_extend(((inst >> 2) & 0x1f) | (cbit(inst, 12) << 5), 6);
                    Self::I {
                        funct: IFunct::ADDI,
                        rd,
                        rs1: rd,
                        imm,
                    }
                }
                0b001 => {
                    // C.JAL
                    let imm = decode_cj_imm(inst);
                    Self::J {
                        funct: JFunct::JAL,
                        rd: 1,
                        imm,
                    }
                }
                0b010 => {
                    // C.LI
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    let imm = sign_extend(((inst >> 2) & 0x1f) | (cbit(inst, 12) << 5), 6);
                    Self::I {
                        funct: IFunct::ADDI,
                        rd,
                        rs1: 0,
                        imm,
                    }
                }
                0b011 => {
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    if rd == 2 {
                        // C.ADDI16SP
                        let nzimm = (cbit(inst, 12) << 9)
                            | (cbit(inst, 6) << 4)
                            | (cbit(inst, 5) << 6)
                            | (((inst >> 3) & 0x3) << 7)
                            | (cbit(inst, 2) << 5);
                        if nzimm == 0 {
                            panic!("illegal C.ADDI16SP");
                        }
                        Self::I {
                            funct: IFunct::ADDI,
                            rd: 2,
                            rs1: 2,
                            imm: sign_extend(nzimm, 10),
                        }
                    } else {
                        // C.LUI
                        let imm6 = ((inst >> 2) & 0x1f) | (cbit(inst, 12) << 5);
                        if rd == 0 || rd == 2 || imm6 == 0 {
                            panic!("illegal C.LUI");
                        }
                        Self::U {
                            funct: UFunct::LUI,
                            rd,
                            imm: sign_extend(imm6, 6) << 12,
                        }
                    }
                }
                0b100 => {
                    let op = (inst >> 10) & 0x3;
                    let rd = 8 + ((inst >> 7) & 0x7) as usize;
                    match op {
                        0b00 => {
                            // C.SRLI
                            if cbit(inst, 12) == 1 {
                                panic!("illegal C.SRLI for RV32");
                            }
                            let shamt = (((inst >> 12) & 0x1) << 5) | ((inst >> 2) & 0x1f);
                            Self::I {
                                funct: IFunct::SRLI,
                                rd,
                                rs1: rd,
                                imm: shamt as i32,
                            }
                        }
                        0b01 => {
                            // C.SRAI
                            if cbit(inst, 12) == 1 {
                                panic!("illegal C.SRAI for RV32");
                            }
                            let shamt = (((inst >> 12) & 0x1) << 5) | ((inst >> 2) & 0x1f);
                            Self::I {
                                funct: IFunct::SRAI,
                                rd,
                                rs1: rd,
                                imm: shamt as i32,
                            }
                        }
                        0b10 => {
                            // C.ANDI
                            let imm = sign_extend(((inst >> 2) & 0x1f) | (cbit(inst, 12) << 5), 6);
                            Self::I {
                                funct: IFunct::ANDI,
                                rd,
                                rs1: rd,
                                imm,
                            }
                        }
                        0b11 => {
                            // C.SUB/C.XOR/C.OR/C.AND
                            if cbit(inst, 12) == 1 {
                                // RV64C uses this space for C.SUBW/C.ADDW.
                                panic!("illegal RV32C ALU op");
                            }
                            let rs2 = 8 + ((inst >> 2) & 0x7) as usize;
                            let funct2 = (inst >> 5) & 0x3;
                            let funct = match funct2 {
                                0b00 => RFunct::SUB,
                                0b01 => RFunct::XOR,
                                0b10 => RFunct::OR,
                                0b11 => RFunct::AND,
                                _ => unreachable!(),
                            };
                            Self::R {
                                funct,
                                rd,
                                rs1: rd,
                                rs2,
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                0b101 => {
                    // C.J
                    let imm = decode_cj_imm(inst);
                    Self::J {
                        funct: JFunct::JAL,
                        rd: 0,
                        imm,
                    }
                }
                0b110 => {
                    // C.BEQZ
                    let rs1 = 8 + ((inst >> 7) & 0x7) as usize;
                    Self::B {
                        funct: BFunct::BEQ,
                        rs1,
                        rs2: 0,
                        imm: decode_cb_imm(inst),
                    }
                }
                0b111 => {
                    // C.BNEZ
                    let rs1 = 8 + ((inst >> 7) & 0x7) as usize;
                    Self::B {
                        funct: BFunct::BNE,
                        rs1,
                        rs2: 0,
                        imm: decode_cb_imm(inst),
                    }
                }
                _ => panic!("illegal compressed instruction (quadrant 1)"),
            },
            0b10 => match funct3 {
                0b000 => {
                    // C.SLLI
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    let shamt = (((inst >> 12) & 0x1) << 5) | ((inst >> 2) & 0x1f);
                    if rd == 0 || cbit(inst, 12) == 1 {
                        panic!("illegal C.SLLI");
                    }
                    Self::I {
                        funct: IFunct::SLLI,
                        rd,
                        rs1: rd,
                        imm: shamt as i32,
                    }
                }
                0b010 => {
                    // C.LWSP
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    if rd == 0 {
                        panic!("illegal C.LWSP");
                    }
                    let uimm = ((inst >> 4) & 0x7) << 2
                        | ((inst >> 12) & 0x1) << 5
                        | ((inst >> 2) & 0x3) << 6;
                    Self::I {
                        funct: IFunct::LW,
                        rd,
                        rs1: 2,
                        imm: uimm as i32,
                    }
                }
                0b011 => {
                    // C.FLWSP
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    if rd == 0 {
                        panic!("illegal C.FLWSP");
                    }
                    let uimm = ((inst >> 4) & 0x7) << 2
                        | ((inst >> 12) & 0x1) << 5
                        | ((inst >> 2) & 0x3) << 6;
                    Self::FL {
                        funct: FLFunct::FLW,
                        rd,
                        rs1: 2,
                        imm: uimm as i32,
                    }
                }
                0b100 => {
                    let bit12 = cbit(inst, 12);
                    let rd = ((inst >> 7) & 0x1f) as usize;
                    let rs2 = ((inst >> 2) & 0x1f) as usize;
                    if bit12 == 0 {
                        if rs2 == 0 {
                            // C.JR
                            if rd == 0 {
                                panic!("illegal C.JR");
                            }
                            Self::I {
                                funct: IFunct::JALR,
                                rd: 0,
                                rs1: rd,
                                imm: 0,
                            }
                        } else {
                            // C.MV
                            if rd == 0 {
                                panic!("illegal C.MV");
                            }
                            Self::R {
                                funct: RFunct::ADD,
                                rd,
                                rs1: 0,
                                rs2,
                            }
                        }
                    } else if rd == 0 && rs2 == 0 {
                        Self::Ebreak
                    } else if rs2 == 0 {
                        // C.JALR
                        if rd == 0 {
                            panic!("illegal C.JALR");
                        }
                        Self::I {
                            funct: IFunct::JALR,
                            rd: 1,
                            rs1: rd,
                            imm: 0,
                        }
                    } else {
                        // C.ADD
                        if rd == 0 {
                            panic!("illegal C.ADD");
                        }
                        Self::R {
                            funct: RFunct::ADD,
                            rd,
                            rs1: rd,
                            rs2,
                        }
                    }
                }
                0b110 => {
                    // C.SWSP
                    let uimm = ((inst >> 9) & 0xf) << 2 | ((inst >> 7) & 0x3) << 6;
                    let rs2 = ((inst >> 2) & 0x1f) as usize;
                    Self::S {
                        funct: SFunct::SW,
                        rs1: 2,
                        rs2,
                        imm: uimm as i32,
                    }
                }
                0b111 => {
                    // C.FSWSP
                    let uimm = ((inst >> 9) & 0xf) << 2 | ((inst >> 7) & 0x3) << 6;
                    let rs2 = ((inst >> 2) & 0x1f) as usize;
                    Self::FS {
                        funct: FSFunct::FSW,
                        rs1: 2,
                        rs2,
                        imm: uimm as i32,
                    }
                }
                _ => panic!("illegal compressed instruction (quadrant 2)"),
            },
            _ => panic!("illegal compressed quadrant"),
        }
    }
}

fn decode_cj_imm(inst: u32) -> i32 {
    let imm = (cbit(inst, 12) << 11)
        | (cbit(inst, 11) << 4)
        | (((inst >> 9) & 0x3) << 8)
        | (cbit(inst, 8) << 10)
        | (cbit(inst, 7) << 6)
        | (cbit(inst, 6) << 7)
        | (((inst >> 3) & 0x7) << 1)
        | (cbit(inst, 2) << 5);
    sign_extend(imm, 12)
}

fn decode_cb_imm(inst: u32) -> i32 {
    let imm = (cbit(inst, 12) << 8)
        | (((inst >> 5) & 0x3) << 6)
        | (cbit(inst, 2) << 5)
        | (((inst >> 10) & 0x3) << 3)
        | (((inst >> 3) & 0x3) << 1);
    sign_extend(imm, 9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mul_as_rv32m() {
        let inst = 0x02b50533; // mul a0, a0, a1
        let (decoded, len) = Instruction::parse_with_len(inst);
        assert_eq!(len, 4);
        match decoded {
            Instruction::M {
                funct: MFunct::MUL,
                rd,
                rs1,
                rs2,
            } => {
                assert_eq!(rd, 10);
                assert_eq!(rs1, 10);
                assert_eq!(rs2, 11);
            }
            _ => panic!("wrong decode"),
        }
    }

    #[test]
    fn parses_amoadd_w() {
        let inst = 0x06b5202f; // amoadd.w.aqrl zero, a1, (a0)
        let (decoded, len) = Instruction::parse_with_len(inst);
        assert_eq!(len, 4);
        match decoded {
            Instruction::A {
                funct: AFunct::AmoAddW,
                rd,
                rs1,
                rs2,
                aq,
                rl,
            } => {
                assert_eq!(rd, 0);
                assert_eq!(rs1, 10);
                assert_eq!(rs2, 11);
                assert!(aq);
                assert!(rl);
            }
            _ => panic!("wrong decode"),
        }
    }

    #[test]
    fn parses_c_jr_with_len_2() {
        let inst = 0x8082; // c.jr ra
        let (decoded, len) = Instruction::parse_with_len(inst);
        assert_eq!(len, 2);
        match decoded {
            Instruction::I {
                funct: IFunct::JALR,
                rd,
                rs1,
                imm,
            } => {
                assert_eq!(rd, 0);
                assert_eq!(rs1, 1);
                assert_eq!(imm, 0);
            }
            _ => panic!("wrong decode"),
        }
    }

    #[test]
    fn parses_c_flwsp_with_len_2() {
        let inst = 0x6092; // c.flwsp f1, 4(sp)
        let (decoded, len) = Instruction::parse_with_len(inst);
        assert_eq!(len, 2);
        match decoded {
            Instruction::FL {
                funct: FLFunct::FLW,
                rd,
                rs1,
                imm,
            } => {
                assert_eq!(rd, 1);
                assert_eq!(rs1, 2);
                assert_eq!(imm, 4);
            }
            _ => panic!("wrong decode"),
        }
    }

    #[test]
    fn parses_fence() {
        let inst = 0x0330_000f;
        let (decoded, len) = Instruction::parse_with_len(inst);
        assert_eq!(len, 4);
        match decoded {
            Instruction::Fence {
                funct: FenceFunct::Fence,
                ..
            } => {}
            _ => panic!("wrong decode"),
        }
    }

    #[test]
    fn parses_flh_fld_and_fsh_fsd() {
        let (decoded, len) = Instruction::parse_with_len(0x0000_9007); // flh f0,0(x1)
        assert_eq!(len, 4);
        match decoded {
            Instruction::FL {
                funct: FLFunct::FLH,
                ..
            } => {}
            _ => panic!("wrong decode"),
        }

        let (decoded, len) = Instruction::parse_with_len(0x0000_b007); // fld f0,0(x1)
        assert_eq!(len, 4);
        match decoded {
            Instruction::FL {
                funct: FLFunct::FLD,
                ..
            } => {}
            _ => panic!("wrong decode"),
        }

        let (decoded, len) = Instruction::parse_with_len(0x0000_9027); // fsh f0,0(x1)
        assert_eq!(len, 4);
        match decoded {
            Instruction::FS {
                funct: FSFunct::FSH,
                ..
            } => {}
            _ => panic!("wrong decode"),
        }

        let (decoded, len) = Instruction::parse_with_len(0x0000_b027); // fsd f0,0(x1)
        assert_eq!(len, 4);
        match decoded {
            Instruction::FS {
                funct: FSFunct::FSD,
                ..
            } => {}
            _ => panic!("wrong decode"),
        }
    }

    #[test]
    fn compressed_sign_extension_regressions() {
        // c.addi a3, -1
        let (decoded, len) = Instruction::parse_with_len(0x16fd);
        assert_eq!(len, 2);
        match decoded {
            Instruction::I {
                funct: IFunct::ADDI,
                rd,
                rs1,
                imm,
            } => {
                assert_eq!(rd, 13);
                assert_eq!(rs1, 13);
                assert_eq!(imm, -1);
            }
            _ => panic!("wrong decode for c.addi"),
        }

        // c.li x5, -1
        let (decoded, len) = Instruction::parse_with_len(0x52fd);
        assert_eq!(len, 2);
        match decoded {
            Instruction::I {
                funct: IFunct::ADDI,
                rd,
                rs1,
                imm,
            } => {
                assert_eq!(rd, 5);
                assert_eq!(rs1, 0);
                assert_eq!(imm, -1);
            }
            _ => panic!("wrong decode for c.li"),
        }

        // c.lui x9, -1
        let (decoded, len) = Instruction::parse_with_len(0x74fd);
        assert_eq!(len, 2);
        match decoded {
            Instruction::U {
                funct: UFunct::LUI,
                rd,
                imm,
            } => {
                assert_eq!(rd, 9);
                assert_eq!(imm, -4096);
            }
            _ => panic!("wrong decode for c.lui"),
        }

        // c.andi x9, -1
        let (decoded, len) = Instruction::parse_with_len(0x98fd);
        assert_eq!(len, 2);
        match decoded {
            Instruction::I {
                funct: IFunct::ANDI,
                rd,
                rs1,
                imm,
            } => {
                assert_eq!(rd, 9);
                assert_eq!(rs1, 9);
                assert_eq!(imm, -1);
            }
            _ => panic!("wrong decode for c.andi"),
        }
    }
}
