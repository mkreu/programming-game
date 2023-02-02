//! The dram module contains a dram structure and implementation for dram access.

/// Default dram size (128MiB).
pub const DRAM_SIZE: u32 = 1024 * 1024 * 128;

/// The address which dram starts, same as QEMU virt machine.
pub const DRAM_BASE: u32 = 0x80_0000;

/// The dynamic random access dram (DRAM).
#[derive(Debug)]
pub struct Dram {
    pub dram: Vec<u8>,
}

use elf::{abi::PT_LOAD, endian::LittleEndian, ElfBytes};
use log::debug;

#[allow(dead_code)]
impl Dram {
    /// Create a new `Dram` instance with default dram size.
    pub fn new(code: Vec<u8>) -> (Dram, u32) {
        let elf = ElfBytes::<LittleEndian>::minimal_parse(&code).expect("failed to parse elf file");

        let all_load_phdrs = elf
            .segments()
            .unwrap()
            .iter()
            .filter(|phdr| phdr.p_type == PT_LOAD)
            .collect::<Vec<_>>();

        let dram_size = all_load_phdrs
            .iter()
            .map(|phdr| phdr.p_vaddr - DRAM_BASE as u64 + phdr.p_memsz)
            .max()
            .unwrap();

        //let mut mem = vec![0u8; dram_size as usize];
        let mut mem = vec![0u8; DRAM_SIZE as usize];

        for phdr in all_load_phdrs {
            let vaddr = phdr.p_vaddr as usize - DRAM_BASE as usize;
            let offset = phdr.p_offset as usize;
            let filesz = phdr.p_filesz as usize;

            println!("vaddr: {vaddr:x}");
            println!("offset: {offset:x}");
            println!("filesz: {filesz:x}");
            mem[DRAM_BASE as usize + vaddr..DRAM_BASE as usize + vaddr + filesz]
                .copy_from_slice(&code[offset..offset + filesz]);
        }

        let entry = elf.ehdr.e_entry;
        debug!("entry: {entry:x}");
        let mut dram = vec![0; DRAM_SIZE as usize];
        dram.splice(..code.len(), code.iter().cloned());

        (Self { dram: mem }, elf.ehdr.e_entry as u32 - DRAM_BASE)
    }

    /// Load bytes from the little-endiam dram.
    pub fn load(&self, addr: u32, size: u32) -> Result<u32, ()> {
        let addr = addr + DRAM_BASE;
        match size {
            8 => Ok(self.load8(addr)),
            16 => Ok(self.load16(addr)),
            32 => Ok(self.load32(addr)),
            _ => Err(()),
        }
    }

    /// Store bytes to the little-endiam dram.
    pub fn store(&mut self, addr: u32, size: u32, value: u32) -> Result<(), ()> {
        let addr = addr + DRAM_BASE;
        match size {
            8 => Ok(self.store8(addr, value)),
            16 => Ok(self.store16(addr, value)),
            32 => Ok(self.store32(addr, value)),
            _ => Err(()),
        }
    }

    /// Load a byte from the little-endian dram.
    fn load8(&self, addr: u32) -> u32 {
        let index = addr as usize;
        self.dram[index] as u32
    }

    /// Load 2 bytes from the little-endian dram.
    fn load16(&self, addr: u32) -> u32 {
        let index = addr as usize;
        return (self.dram[index] as u32) | ((self.dram[index + 1] as u32) << 8);
    }

    /// Load 4 bytes from the little-endian dram.
    fn load32(&self, addr: u32) -> u32 {
        let index = addr as usize;
        return (self.dram[index] as u32)
            | ((self.dram[index + 1] as u32) << 8)
            | ((self.dram[index + 2] as u32) << 16)
            | ((self.dram[index + 3] as u32) << 24);
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
