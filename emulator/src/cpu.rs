use elf::{ElfBytes, abi::PT_LOAD, endian::LittleEndian};
use tracing::{debug, trace};

pub use instruction::Instruction;
mod instruction;

#[derive(Debug)]
pub struct Hart {
    pub regs: [u32; 32],
    pub pc: u32,
}

impl Hart {
    pub fn new(entry: u32) -> Self {
        let mut cpu = Self {
            regs: [0; 32],
            pc: entry,
        };
        cpu.regs[2] = DRAM_SIZE - 8;
        cpu
    }
    pub fn fetch(&self, dram: &impl RamLike) -> u32 {
        dram.load(self.pc, 32).unwrap()
    }
    pub fn execute(&mut self, inst: Instruction, dram: &mut impl RamLike) {
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
                    self.regs[rd] = ((dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 8)
                        .unwrap()
                        << 24) as i32
                        >> 24) as u32;
                }
                instruction::IFunct::LH => {
                    self.regs[rd] = ((dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 16)
                        .unwrap()
                        << 16) as i32
                        >> 16) as u32;
                }
                instruction::IFunct::LW => {
                    self.regs[rd] = dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 32)
                        .unwrap();
                }
                instruction::IFunct::LBU => {
                    self.regs[rd] = dram
                        .load(self.regs[rs1].wrapping_add_signed(imm), 8)
                        .unwrap();
                }
                instruction::IFunct::LHU => {
                    self.regs[rd] = dram
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
                    dram.store(self.regs[rs1].wrapping_add_signed(imm), 8, self.regs[rs2])
                        .unwrap();
                }
                instruction::SFunct::SH => {
                    dram.store(self.regs[rs1].wrapping_add_signed(imm), 16, self.regs[rs2])
                        .unwrap();
                }
                instruction::SFunct::SW => {
                    dram.store(self.regs[rs1].wrapping_add_signed(imm), 32, self.regs[rs2])
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

/// Default dram size (64KiB).
pub const DRAM_SIZE: u32 = 1024 * 64;

pub trait RamLike {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()>;
    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()>;
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
        let addr = addr;
        match size {
            8 => Ok(self.load8(addr)),
            16 => Ok(self.load16(addr)),
            32 => Ok(self.load32(addr)),
            _ => Err(()),
        }
    }

    /// Store bytes to the little-endiam dram.
    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        trace!("store(addr: {addr:x}, size: {size})");
        let addr = addr;
        match size {
            8 => {
                self.store8(addr, value);
                Ok(())
            }
            16 => {
                self.store16(addr, value);
                Ok(())
            }
            32 => {
                self.store32(addr, value);
                Ok(())
            }
            _ => Err(()),
        }
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

        //let dram_size = all_load_phdrs
        //    .iter()
        //    .map(|phdr| phdr.p_vaddr - DRAM_BASE as u64 + phdr.p_memsz)
        //    .max()
        //    .unwrap();

        //let mut mem = vec![0u8; dram_size as usize];
        let mut mem = vec![0u8; DRAM_SIZE as usize];

        for phdr in all_load_phdrs {
            let vaddr = phdr.p_vaddr as usize;
            let offset = phdr.p_offset as usize;
            let filesz = phdr.p_filesz as usize;

            println!("vaddr: {vaddr:x}");
            println!("offset: {offset:x}");
            println!("filesz: {filesz:x}");
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

pub struct Mmu {
    dram: Dram,
}

impl RamLike for Mmu {
    fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        self.dram.load(addr, size)
    }

    fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        self.dram.store(addr, size, value)
    }
}