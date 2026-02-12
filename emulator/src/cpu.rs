use std::any::Any;

use elf::{ElfBytes, abi::PT_LOAD, endian::LittleEndian};
use tracing::{debug, trace};

pub use instruction::Instruction;
mod instruction;

#[derive(Debug)]
pub struct Hart {
    pub regs: [u32; 32],
    pub fregs: [u32; 32],
    pub pc: u32,
    pub reservation_addr: Option<u32>,
}

impl Hart {
    pub fn new(entry: u32) -> Self {
        let mut cpu = Self {
            regs: [0; 32],
            fregs: [0; 32],
            pc: entry,
            reservation_addr: None,
        };
        cpu.regs[2] = (DRAM_SIZE - 16) & !0xf;
        cpu
    }
    pub fn fetch(&self, dram: &impl RamLike) -> u32 {
        dram.load(self.pc, 32).unwrap_or(0)
    }
    pub fn set_reservation(&mut self, addr: u32) {
        self.reservation_addr = Some(addr);
    }

    pub fn clear_reservation(&mut self) {
        self.reservation_addr = None;
    }

    fn invalidate_reservation_if_overlaps(&mut self, addr: u32) {
        if let Some(res) = self.reservation_addr {
            // LR/SC reservation is word-granular for this simplified RV32A model.
            if (addr & !0x3) == (res & !0x3) {
                self.clear_reservation();
            }
        }
    }

    pub fn execute(&mut self, inst: Instruction, inst_len: u32, dram: &mut impl RamLike) {
        self.regs[0] = 0; // Simulate hard wired x0
        self.pc = self.pc.wrapping_add(inst_len);

        fn f32_from_bits(bits: u32) -> f32 {
            f32::from_bits(bits)
        }

        fn bits_from_f32(value: f32) -> u32 {
            value.to_bits()
        }

        fn is_nan_bits(bits: u32) -> bool {
            let exp = (bits >> 23) & 0xff;
            let frac = bits & 0x7f_ffff;
            exp == 0xff && frac != 0
        }

        fn fclass_s(bits: u32) -> u32 {
            let sign = (bits >> 31) != 0;
            let exp = (bits >> 23) & 0xff;
            let frac = bits & 0x7f_ffff;
            if exp == 0xff {
                if frac == 0 {
                    if sign { 1 << 0 } else { 1 << 7 }
                } else {
                    // Quiet NaN when the top mantissa bit is set, signaling otherwise.
                    let is_quiet = ((frac >> 22) & 0x1) == 1;
                    if is_quiet { 1 << 9 } else { 1 << 8 }
                }
            } else if exp == 0 {
                if frac == 0 {
                    if sign { 1 << 3 } else { 1 << 4 }
                } else if sign {
                    1 << 2
                } else {
                    1 << 5
                }
            } else if sign {
                1 << 1
            } else {
                1 << 6
            }
        }

        fn fcvt_w_s(value: f32, unsigned: bool) -> u32 {
            if value.is_nan() {
                return 0;
            }
            if unsigned {
                if value <= 0.0 {
                    0
                } else if value >= u32::MAX as f32 {
                    u32::MAX
                } else {
                    value.trunc() as u32
                }
            } else if value <= i32::MIN as f32 {
                i32::MIN as u32
            } else if value >= i32::MAX as f32 {
                i32::MAX as u32
            } else {
                (value.trunc() as i32) as u32
            }
        }

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
                    self.regs[rd] = ((self.regs[rs1] as i32) >> shamt) as u32;
                }
                instruction::RFunct::OR => {
                    self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                }
                instruction::RFunct::AND => {
                    self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                }
            },
            Instruction::M {
                funct,
                rd,
                rs1,
                rs2,
            } => match funct {
                instruction::MFunct::MUL => {
                    self.regs[rd] = self.regs[rs1].wrapping_mul(self.regs[rs2]);
                }
                instruction::MFunct::MULH => {
                    let lhs = self.regs[rs1] as i32 as i64;
                    let rhs = self.regs[rs2] as i32 as i64;
                    self.regs[rd] = ((lhs.wrapping_mul(rhs) >> 32) & 0xffff_ffff) as u32;
                }
                instruction::MFunct::MULHSU => {
                    let lhs = self.regs[rs1] as i32 as i64;
                    let rhs = self.regs[rs2] as u64 as i64;
                    self.regs[rd] = ((lhs.wrapping_mul(rhs) >> 32) & 0xffff_ffff) as u32;
                }
                instruction::MFunct::MULHU => {
                    let lhs = self.regs[rs1] as u64;
                    let rhs = self.regs[rs2] as u64;
                    self.regs[rd] = ((lhs.wrapping_mul(rhs) >> 32) & 0xffff_ffff) as u32;
                }
                instruction::MFunct::DIV => {
                    let lhs = self.regs[rs1] as i32;
                    let rhs = self.regs[rs2] as i32;
                    self.regs[rd] = if rhs == 0 {
                        u32::MAX
                    } else if lhs == i32::MIN && rhs == -1 {
                        i32::MIN as u32
                    } else {
                        lhs.wrapping_div(rhs) as u32
                    };
                }
                instruction::MFunct::DIVU => {
                    let lhs = self.regs[rs1];
                    let rhs = self.regs[rs2];
                    self.regs[rd] = if rhs == 0 {
                        u32::MAX
                    } else {
                        lhs.wrapping_div(rhs)
                    };
                }
                instruction::MFunct::REM => {
                    let lhs = self.regs[rs1] as i32;
                    let rhs = self.regs[rs2] as i32;
                    self.regs[rd] = if rhs == 0 {
                        lhs as u32
                    } else if lhs == i32::MIN && rhs == -1 {
                        0
                    } else {
                        lhs.wrapping_rem(rhs) as u32
                    };
                }
                instruction::MFunct::REMU => {
                    let lhs = self.regs[rs1];
                    let rhs = self.regs[rs2];
                    self.regs[rd] = if rhs == 0 {
                        lhs
                    } else {
                        lhs.wrapping_rem(rhs)
                    };
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
                    self.regs[rd] = ((dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 8)
                        .unwrap_or(0)
                        << 24) as i32
                        >> 24) as u32;
                }
                instruction::IFunct::LH => {
                    self.regs[rd] = ((dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 16)
                        .unwrap_or(0)
                        << 16) as i32
                        >> 16) as u32;
                }
                instruction::IFunct::LW => {
                    self.regs[rd] = dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 32)
                        .unwrap_or(0);
                }
                instruction::IFunct::LBU => {
                    self.regs[rd] = dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 8)
                        .unwrap_or(0);
                }
                instruction::IFunct::LHU => {
                    self.regs[rd] = dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 16)
                        .unwrap_or(0);
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
                    let shamt = (imm as u32) & 0x1f;
                    self.regs[rd] = self.regs[rs1] << shamt;
                }
                instruction::IFunct::SRLI => {
                    let shamt = (imm as u32) & 0x1f;
                    self.regs[rd] = self.regs[rs1] >> shamt;
                }
                instruction::IFunct::SRAI => {
                    let shamt = (imm as u32) & 0x1f;
                    self.regs[rd] = ((self.regs[rs1] as i32) >> shamt) as u32;
                }
            },
            Instruction::S {
                funct,
                rs1,
                rs2,
                imm,
            } => match funct {
                instruction::SFunct::SB => {
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    let _ = dram.store(addr, 8, self.regs[rs2]);
                    self.invalidate_reservation_if_overlaps(addr);
                }
                instruction::SFunct::SH => {
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    let _ = dram.store(addr, 16, self.regs[rs2]);
                    self.invalidate_reservation_if_overlaps(addr);
                }
                instruction::SFunct::SW => {
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    let _ = dram.store(addr, 32, self.regs[rs2]);
                    self.invalidate_reservation_if_overlaps(addr);
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
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                    }
                }
                instruction::BFunct::BNE => {
                    if self.regs[rs1] != self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                    }
                }
                instruction::BFunct::BLT => {
                    if (self.regs[rs1] as i32) < (self.regs[rs2] as i32) {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                    }
                }
                instruction::BFunct::BGE => {
                    if (self.regs[rs1] as i32) >= (self.regs[rs2] as i32) {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                    }
                }
                instruction::BFunct::BLTU => {
                    if self.regs[rs1] < self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                    }
                }
                instruction::BFunct::BGEU => {
                    if self.regs[rs1] >= self.regs[rs2] {
                        self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                    }
                }
            },
            Instruction::U { funct, rd, imm } => match funct {
                instruction::UFunct::LUI => {
                    self.regs[rd] = imm as u32;
                }
                instruction::UFunct::AUIPC => {
                    self.regs[rd] = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                }
            },
            Instruction::J { funct, rd, imm } => match funct {
                instruction::JFunct::JAL => {
                    self.regs[rd] = self.pc;
                    self.pc = self.pc.wrapping_add_signed(imm).wrapping_sub(inst_len);
                }
            },
            Instruction::R4 {
                funct,
                rd,
                rs1,
                rs2,
                rs3,
                rm: _,
            } => {
                let a = f32_from_bits(self.fregs[rs1]);
                let b = f32_from_bits(self.fregs[rs2]);
                let c = f32_from_bits(self.fregs[rs3]);
                let result = match funct {
                    instruction::R4Funct::FmaddS => a.mul_add(b, c),
                    instruction::R4Funct::FmsubS => a.mul_add(b, -c),
                    instruction::R4Funct::FnmsubS => (-a).mul_add(b, c),
                    instruction::R4Funct::FnmaddS => (-a).mul_add(b, -c),
                };
                self.fregs[rd] = bits_from_f32(result);
            }
            Instruction::FR {
                funct,
                rd,
                rs1,
                rs2,
                rm: _,
            } => match funct {
                instruction::FRFunct::FaddS => {
                    let result = f32_from_bits(self.fregs[rs1]) + f32_from_bits(self.fregs[rs2]);
                    self.fregs[rd] = bits_from_f32(result);
                }
                instruction::FRFunct::FsubS => {
                    let result = f32_from_bits(self.fregs[rs1]) - f32_from_bits(self.fregs[rs2]);
                    self.fregs[rd] = bits_from_f32(result);
                }
                instruction::FRFunct::FmulS => {
                    let result = f32_from_bits(self.fregs[rs1]) * f32_from_bits(self.fregs[rs2]);
                    self.fregs[rd] = bits_from_f32(result);
                }
                instruction::FRFunct::FdivS => {
                    let result = f32_from_bits(self.fregs[rs1]) / f32_from_bits(self.fregs[rs2]);
                    self.fregs[rd] = bits_from_f32(result);
                }
                instruction::FRFunct::FsgnjS => {
                    self.fregs[rd] = (self.fregs[rs1] & 0x7fff_ffff) | (self.fregs[rs2] & 0x8000_0000);
                }
                instruction::FRFunct::FsgnjnS => {
                    self.fregs[rd] =
                        (self.fregs[rs1] & 0x7fff_ffff) | ((!self.fregs[rs2]) & 0x8000_0000);
                }
                instruction::FRFunct::FsgnjxS => {
                    let sign = (self.fregs[rs1] ^ self.fregs[rs2]) & 0x8000_0000;
                    self.fregs[rd] = (self.fregs[rs1] & 0x7fff_ffff) | sign;
                }
                instruction::FRFunct::FminS => {
                    let a_bits = self.fregs[rs1];
                    let b_bits = self.fregs[rs2];
                    let a = f32_from_bits(a_bits);
                    let b = f32_from_bits(b_bits);
                    self.fregs[rd] = if a.is_nan() && b.is_nan() {
                        f32::NAN.to_bits()
                    } else if a.is_nan() {
                        b_bits
                    } else if b.is_nan() {
                        a_bits
                    } else if a < b {
                        a_bits
                    } else if b < a {
                        b_bits
                    } else if a_bits == 0x8000_0000 || b_bits == 0x8000_0000 {
                        0x8000_0000
                    } else {
                        a_bits
                    };
                }
                instruction::FRFunct::FmaxS => {
                    let a_bits = self.fregs[rs1];
                    let b_bits = self.fregs[rs2];
                    let a = f32_from_bits(a_bits);
                    let b = f32_from_bits(b_bits);
                    self.fregs[rd] = if a.is_nan() && b.is_nan() {
                        f32::NAN.to_bits()
                    } else if a.is_nan() {
                        b_bits
                    } else if b.is_nan() {
                        a_bits
                    } else if a > b {
                        a_bits
                    } else if b > a {
                        b_bits
                    } else if a_bits == 0x0000_0000 || b_bits == 0x0000_0000 {
                        0x0000_0000
                    } else {
                        a_bits
                    };
                }
                instruction::FRFunct::FeqS => {
                    let a_bits = self.fregs[rs1];
                    let b_bits = self.fregs[rs2];
                    self.regs[rd] = if is_nan_bits(a_bits) || is_nan_bits(b_bits) {
                        0
                    } else if f32_from_bits(a_bits) == f32_from_bits(b_bits) {
                        1
                    } else {
                        0
                    };
                }
                instruction::FRFunct::FltS => {
                    let a_bits = self.fregs[rs1];
                    let b_bits = self.fregs[rs2];
                    self.regs[rd] = if is_nan_bits(a_bits) || is_nan_bits(b_bits) {
                        0
                    } else if f32_from_bits(a_bits) < f32_from_bits(b_bits) {
                        1
                    } else {
                        0
                    };
                }
                instruction::FRFunct::FleS => {
                    let a_bits = self.fregs[rs1];
                    let b_bits = self.fregs[rs2];
                    self.regs[rd] = if is_nan_bits(a_bits) || is_nan_bits(b_bits) {
                        0
                    } else if f32_from_bits(a_bits) <= f32_from_bits(b_bits) {
                        1
                    } else {
                        0
                    };
                }
            },
            Instruction::FI {
                funct,
                rd,
                rs1,
                rm: _,
            } => match funct {
                instruction::FIFunct::FsqrtS => {
                    self.fregs[rd] = bits_from_f32(f32_from_bits(self.fregs[rs1]).sqrt());
                }
                instruction::FIFunct::FcvtWS => {
                    self.regs[rd] = fcvt_w_s(f32_from_bits(self.fregs[rs1]), false);
                }
                instruction::FIFunct::FcvtWuS => {
                    self.regs[rd] = fcvt_w_s(f32_from_bits(self.fregs[rs1]), true);
                }
                instruction::FIFunct::FmvXW => {
                    self.regs[rd] = self.fregs[rs1];
                }
                instruction::FIFunct::FclassS => {
                    self.regs[rd] = fclass_s(self.fregs[rs1]);
                }
                instruction::FIFunct::FcvtSW => {
                    self.fregs[rd] = bits_from_f32((self.regs[rs1] as i32) as f32);
                }
                instruction::FIFunct::FcvtSWU => {
                    self.fregs[rd] = bits_from_f32(self.regs[rs1] as f32);
                }
                instruction::FIFunct::FmvWX => {
                    self.fregs[rd] = self.regs[rs1];
                }
            },
            Instruction::FL {
                funct,
                rd,
                rs1,
                imm,
            } => match funct {
                instruction::FLFunct::FLH => {
                    // Minimal half-precision support: keep payload in low 16 bits.
                    // Proper IEEE half handling/NaN-boxing is out of scope for this phase.
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    self.fregs[rd] = match dram.load(addr, 16) {
                        Ok(v) => v & 0xffff,
                        Err(()) => 0,
                    };
                }
                instruction::FLFunct::FLW => {
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    self.fregs[rd] = dram.load(addr, 32).unwrap_or(0);
                }
                instruction::FLFunct::FLD => {
                    // Single-precision register model: consume 64-bit memory access but
                    // preserve only low 32 bits in fregs.
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    self.fregs[rd] = dram.load(addr, 32).unwrap_or(0);
                    let _ = dram.load(addr.wrapping_add(4), 32);
                }
            },
            Instruction::FS {
                funct,
                rs1,
                rs2,
                imm,
            } => match funct {
                instruction::FSFunct::FSH => {
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    let _ = dram.store(addr, 16, self.fregs[rs2]);
                    self.invalidate_reservation_if_overlaps(addr);
                }
                instruction::FSFunct::FSW => {
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    let _ = dram.store(addr, 32, self.fregs[rs2]);
                    self.invalidate_reservation_if_overlaps(addr);
                }
                instruction::FSFunct::FSD => {
                    // Single-precision register model: write low 32 bits and NaN-box upper.
                    let addr = self.regs[rs1].wrapping_add_signed(imm);
                    let _ = dram.store(addr, 32, self.fregs[rs2]);
                    let _ = dram.store(addr.wrapping_add(4), 32, u32::MAX);
                    self.invalidate_reservation_if_overlaps(addr);
                }
            },
            Instruction::A {
                funct,
                rd,
                rs1,
                rs2,
                aq: _,
                rl: _,
            } => {
                let addr = self.regs[rs1];
                match funct {
                    instruction::AFunct::LrW => {
                        self.regs[rd] = dram.load(addr, 32).unwrap_or(0);
                        self.set_reservation(addr);
                    }
                    instruction::AFunct::ScW => {
                        let success = self.reservation_addr == Some(addr);
                        if success {
                            let _ = dram.store(addr, 32, self.regs[rs2]);
                        }
                        self.regs[rd] = if success { 0 } else { 1 };
                        self.clear_reservation();
                    }
                    _ => {
                        let old = dram.load(addr, 32).unwrap_or(0);
                        let rhs = self.regs[rs2];
                        let new = match funct {
                            instruction::AFunct::AmoSwapW => rhs,
                            instruction::AFunct::AmoAddW => old.wrapping_add(rhs),
                            instruction::AFunct::AmoXorW => old ^ rhs,
                            instruction::AFunct::AmoAndW => old & rhs,
                            instruction::AFunct::AmoOrW => old | rhs,
                            instruction::AFunct::AmoMinW => {
                                if (old as i32) < (rhs as i32) {
                                    old
                                } else {
                                    rhs
                                }
                            }
                            instruction::AFunct::AmoMaxW => {
                                if (old as i32) > (rhs as i32) {
                                    old
                                } else {
                                    rhs
                                }
                            }
                            instruction::AFunct::AmoMinuW => {
                                if old < rhs {
                                    old
                                } else {
                                    rhs
                                }
                            }
                            instruction::AFunct::AmoMaxuW => {
                                if old > rhs {
                                    old
                                } else {
                                    rhs
                                }
                            }
                            instruction::AFunct::LrW | instruction::AFunct::ScW => unreachable!(),
                        };
                        let _ = dram.store(addr, 32, new);
                        self.regs[rd] = old;
                        self.invalidate_reservation_if_overlaps(addr);
                    }
                }
            }
            Instruction::Fence {
                funct: _,
                pred: _,
                succ: _,
                fm: _,
            } => {
                // Single-hart simplified model: fence/fence.i are no-ops.
            }
            Instruction::Ebreak => {
                panic!("ebreak");
            }
        }
        self.regs[0] = 0;
    }
}

/// Minimum DRAM size (64KiB).
pub const DRAM_SIZE: u32 = 1024 * 64;
/// Stack headroom reserved above loaded ELF segments.
pub const STACK_HEADROOM: u32 = 1024 * 256;

pub trait RamLike: Send + Sync {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()>;
    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()>;

    /// Support downcasting to concrete types.
    fn as_any(&self) -> &dyn Any;
    /// Support downcasting to concrete types (mutable).
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// The dynamic random access dram (DRAM).
#[derive(Debug)]
pub struct Dram {
    pub dram: Vec<u8>,
}

#[allow(dead_code)]
impl RamLike for Dram {
    /// Load bytes from the little-endiam dram.
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        trace!("load(addr: {addr:x}, size: {size})");
        let addr = addr as usize;
        let width = match size {
            8 => 1usize,
            16 => 2usize,
            32 => 4usize,
            _ => return Err(()),
        };
        if addr.checked_add(width).is_none_or(|end| end > self.dram.len()) {
            return Err(());
        }
        match size {
            8 => Ok(self.load8(addr as u32)),
            16 => Ok(self.load16(addr as u32)),
            32 => Ok(self.load32(addr as u32)),
            _ => unreachable!(),
        }
    }

    /// Store bytes to the little-endiam dram.
    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        trace!("store(addr: {addr:x}, size: {size})");
        let addr = addr as usize;
        let width = match size {
            8 => 1usize,
            16 => 2usize,
            32 => 4usize,
            _ => return Err(()),
        };
        if addr.checked_add(width).is_none_or(|end| end > self.dram.len()) {
            return Err(());
        }
        match size {
            8 => {
                self.store8(addr as u32, value);
                Ok(())
            }
            16 => {
                self.store16(addr as u32, value);
                Ok(())
            }
            32 => {
                self.store32(addr as u32, value);
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Dram {
    /// Create a new `Dram` instance with default dram size.
    pub fn new(code: &[u8]) -> (Dram, u32) {
        let elf = ElfBytes::<LittleEndian>::minimal_parse(code).expect("failed to parse elf file");

        let all_load_phdrs = elf
            .segments()
            .unwrap()
            .iter()
            .filter(|phdr| phdr.p_type == PT_LOAD)
            .collect::<Vec<_>>();

        let max_load_end = all_load_phdrs
            .iter()
            .map(|phdr| phdr.p_vaddr as u32 + phdr.p_memsz as u32)
            .max()
            .unwrap_or(0);
        let required = max_load_end.saturating_add(STACK_HEADROOM).max(DRAM_SIZE);
        let dram_size = ((required + 0xf) & !0xf) as usize;
        let mut mem = vec![0u8; dram_size];

        for phdr in all_load_phdrs {
            let vaddr = phdr.p_vaddr as usize;
            let offset = phdr.p_offset as usize;
            let filesz = phdr.p_filesz as usize;

            mem[vaddr..vaddr + filesz].copy_from_slice(&code[offset..offset + filesz]);
        }

        let entry = elf.ehdr.e_entry as u32;
        debug!("entry: {entry:x}");
        (Self { dram: mem }, entry)
    }

    /// Load a byte from the little-endian dram.
    fn load8(&self, addr: u32) -> u32 {
        let index = addr as usize;
        self.dram[index] as u32
    }

    /// Load 2 bytes from the little-endian dram.
    fn load16(&self, addr: u32) -> u32 {
        let index = addr as usize;
        (self.dram[index] as u32) | ((self.dram[index + 1] as u32) << 8)
    }

    /// Load 4 bytes from the little-endian dram.
    fn load32(&self, addr: u32) -> u32 {
        let index = addr as usize;
        (self.dram[index] as u32)
            | ((self.dram[index + 1] as u32) << 8)
            | ((self.dram[index + 2] as u32) << 16)
            | ((self.dram[index + 3] as u32) << 24)
    }

    /// Store a byte to the little-endian dram.
    fn store8(&mut self, addr: u32, value: u32) {
        let index = addr as usize;
        self.dram[index] = value as u8
    }

    /// Store 2 bytes to the little-endian dram.
    fn store16(&mut self, addr: u32, value: u32) {
        let index = addr as usize;
        self.dram[index] = (value & 0xff) as u8;
        self.dram[index + 1] = ((value >> 8) & 0xff) as u8;
    }

    /// Store 4 bytes to the little-endian dram.
    fn store32(&mut self, addr: u32, value: u32) {
        let index = addr as usize;
        self.dram[index] = (value & 0xff) as u8;
        self.dram[index + 1] = ((value >> 8) & 0xff) as u8;
        self.dram[index + 2] = ((value >> 16) & 0xff) as u8;
        self.dram[index + 3] = ((value >> 24) & 0xff) as u8;
    }
}

pub struct Mmu<'a> {
    pub dram: &'a mut Dram,
    pub devices: &'a mut [&'a mut dyn RamLike],
}

impl<'a> Mmu<'a> {
    pub fn new(dram: &'a mut Dram, devices: &'a mut [&'a mut dyn RamLike]) -> Self {
        Self { dram, devices }
    }
}

impl RamLike for Mmu<'_> {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        if addr >= 0x1000 {
            self.dram.load(addr, size)
        } else if addr >= 0x100 {
            let device_index = ((addr >> 8) & 0xF) as usize - 1;
            if let Some(device) = self.devices.get(device_index) {
                device.load(addr & 0xFF, size)
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        if addr >= 0x1000 {
            self.dram.store(addr, size, value)
        } else if addr >= 0x100 {
            let device_index = ((addr >> 8) & 0xF) as usize - 1;
            if let Some(device) = self.devices.get_mut(device_index) {
                device.store(addr & 0xFF, size, value)
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    fn as_any(&self) -> &dyn Any {
        unimplemented!("Mmu does not support downcasting")
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        unimplemented!("Mmu does not support downcasting")
    }
}

pub struct LogDevice;

impl RamLike for LogDevice {
    fn load(&self, _addr: u32, _size: u32) -> Result<u32, ()> {
        Ok(0)
    }

    fn store(&mut self, _addr: u32, size: u32, value: u32) -> Result<(), ()> {
        if size != 32 {
            return Err(());
        }
        print!("{}", char::from_u32(value).unwrap());
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::instruction::{
        AFunct, FLFunct, FSFunct, IFunct, Instruction, MFunct, SFunct,
    };

    struct TestRam {
        bytes: Vec<u8>,
    }

    impl TestRam {
        fn new(size: usize) -> Self {
            Self {
                bytes: vec![0; size],
            }
        }
    }

    impl RamLike for TestRam {
        fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
            let i = addr as usize;
            Ok(match size {
                8 => self.bytes[i] as u32,
                16 => (self.bytes[i] as u32) | ((self.bytes[i + 1] as u32) << 8),
                32 => {
                    (self.bytes[i] as u32)
                        | ((self.bytes[i + 1] as u32) << 8)
                        | ((self.bytes[i + 2] as u32) << 16)
                        | ((self.bytes[i + 3] as u32) << 24)
                }
                _ => return Err(()),
            })
        }

        fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
            let i = addr as usize;
            match size {
                8 => self.bytes[i] = value as u8,
                16 => {
                    self.bytes[i] = (value & 0xff) as u8;
                    self.bytes[i + 1] = ((value >> 8) & 0xff) as u8;
                }
                32 => {
                    self.bytes[i] = (value & 0xff) as u8;
                    self.bytes[i + 1] = ((value >> 8) & 0xff) as u8;
                    self.bytes[i + 2] = ((value >> 16) & 0xff) as u8;
                    self.bytes[i + 3] = ((value >> 24) & 0xff) as u8;
                }
                _ => return Err(()),
            }
            Ok(())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn m_div_edge_cases() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);

        h.regs[1] = i32::MIN as u32;
        h.regs[2] = u32::MAX; // -1
        h.execute(
            Instruction::M {
                funct: MFunct::DIV,
                rd: 3,
                rs1: 1,
                rs2: 2,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.regs[3], i32::MIN as u32);

        h.regs[2] = 0;
        h.execute(
            Instruction::M {
                funct: MFunct::DIVU,
                rd: 4,
                rs1: 1,
                rs2: 2,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.regs[4], u32::MAX);
    }

    #[test]
    fn a_lr_sc_success_and_fail() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);
        ram.store(100, 32, 10).unwrap();
        h.regs[1] = 100;
        h.regs[2] = 55;

        h.execute(
            Instruction::A {
                funct: AFunct::LrW,
                rd: 3,
                rs1: 1,
                rs2: 0,
                aq: false,
                rl: false,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.regs[3], 10);

        h.execute(
            Instruction::A {
                funct: AFunct::ScW,
                rd: 4,
                rs1: 1,
                rs2: 2,
                aq: false,
                rl: false,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.regs[4], 0);
        assert_eq!(ram.load(100, 32).unwrap(), 55);

        // Single-hart simplified invalidation: any store clears reservation.
        h.execute(
            Instruction::A {
                funct: AFunct::LrW,
                rd: 3,
                rs1: 1,
                rs2: 0,
                aq: false,
                rl: false,
            },
            4,
            &mut ram,
        );
        h.execute(
            Instruction::S {
                funct: SFunct::SW,
                rs1: 1,
                rs2: 2,
                imm: 0,
            },
            4,
            &mut ram,
        );
        h.execute(
            Instruction::A {
                funct: AFunct::ScW,
                rd: 5,
                rs1: 1,
                rs2: 2,
                aq: false,
                rl: false,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.regs[5], 1);
    }

    #[test]
    fn a_amoadd() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);
        h.regs[1] = 100;
        h.regs[2] = 7;
        ram.store(100, 32, 3).unwrap();

        h.execute(
            Instruction::A {
                funct: AFunct::AmoAddW,
                rd: 4,
                rs1: 1,
                rs2: 2,
                aq: true,
                rl: true,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.regs[4], 3);
        assert_eq!(ram.load(100, 32).unwrap(), 10);
    }

    #[test]
    fn compressed_jr_respects_2byte_len() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);
        h.pc = 100;
        h.regs[1] = 200;
        let (inst, len) = Instruction::parse_with_len(0x8082); // c.jr ra
        h.execute(inst, len, &mut ram);
        assert_eq!(h.pc, 200);
    }

    #[test]
    fn compressed_zcf_stack_load_store() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);
        h.regs[2] = 128;
        h.fregs[1] = 0x3f80_0000;

        let fswsp = Instruction::parse_with_len(0xe206).0; // c.fswsp f1, 4(sp)
        h.execute(fswsp, 2, &mut ram);
        assert_eq!(ram.load(132, 32).unwrap(), 0x3f80_0000);

        h.fregs[1] = 0;
        let flwsp = Instruction::parse_with_len(0x6092).0; // c.flwsp f1, 4(sp)
        h.execute(
            Instruction::FL {
                funct: FLFunct::FLW,
                rd: 1,
                rs1: 2,
                imm: 4,
            },
            4,
            &mut ram,
        );
        h.execute(flwsp, 2, &mut ram);
        assert_eq!(h.fregs[1], 0x3f80_0000);
    }

    #[test]
    fn fp_load_store_variants_roundtrip_memory() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);
        h.regs[1] = 200;
        h.fregs[2] = 0x1234_5678;

        h.execute(
            Instruction::FS {
                funct: FSFunct::FSH,
                rs1: 1,
                rs2: 2,
                imm: 0,
            },
            4,
            &mut ram,
        );
        assert_eq!(ram.load(200, 16).unwrap(), 0x5678);

        h.fregs[3] = 0;
        h.execute(
            Instruction::FL {
                funct: FLFunct::FLH,
                rd: 3,
                rs1: 1,
                imm: 0,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.fregs[3], 0x5678);

        h.execute(
            Instruction::FS {
                funct: FSFunct::FSD,
                rs1: 1,
                rs2: 2,
                imm: 8,
            },
            4,
            &mut ram,
        );
        assert_eq!(ram.load(208, 32).unwrap(), 0x1234_5678);
        assert_eq!(ram.load(212, 32).unwrap(), u32::MAX);

        h.fregs[4] = 0;
        h.execute(
            Instruction::FL {
                funct: FLFunct::FLD,
                rd: 4,
                rs1: 1,
                imm: 8,
            },
            4,
            &mut ram,
        );
        assert_eq!(h.fregs[4], 0x1234_5678);
    }

    #[test]
    fn parse_compressed_addi_executes() {
        let mut h = Hart::new(0);
        let mut ram = TestRam::new(1024);
        h.regs[1] = 5;
        let (inst, len) = Instruction::parse_with_len(0x0085); // c.addi x1, 1
        match inst {
            Instruction::I {
                funct: IFunct::ADDI,
                ..
            } => {}
            _ => panic!("unexpected decode"),
        }
        h.execute(inst, len, &mut ram);
        assert_eq!(h.regs[1], 6);
    }

    #[test]
    fn parse_compressed_lw_swsp_variants() {
        let (inst, len) = Instruction::parse_with_len(0xc20c); // representative c.sw
        assert_eq!(len, 2);
        match inst {
            Instruction::S {
                funct: SFunct::SW, ..
            } => {}
            _ => panic!("unexpected decode"),
        }

        let (inst, len) = Instruction::parse_with_len(0x4102); // representative c.lwsp
        assert_eq!(len, 2);
        match inst {
            Instruction::I {
                funct: IFunct::LW, ..
            } => {}
            _ => panic!("unexpected decode"),
        }

        let (inst, len) = Instruction::parse_with_len(0xc006); // representative c.swsp
        assert_eq!(len, 2);
        match inst {
            Instruction::S {
                funct: SFunct::SW, ..
            } => {}
            _ => panic!("unexpected decode"),
        }

        let (inst, len) = Instruction::parse_with_len(0xe206); // c.fswsp
        assert_eq!(len, 2);
        match inst {
            Instruction::FS {
                funct: FSFunct::FSW,
                ..
            } => {}
            _ => panic!("unexpected decode"),
        }
    }
}
