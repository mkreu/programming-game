use bevy::prelude::*;

use crate::CpuBuilder;
use crate::cpu::{Instruction, Mmu, RamLike};

#[derive(Component)]
pub struct CpuComponent {
    hart: crate::cpu::Hart,
    dram: crate::cpu::Dram,
    devices: Vec<Box<dyn RamLike>>,
    instructions_per_update: u32,
}

impl CpuComponent {
    /// Create a new CpuComponent from an ELF binary.
    /// `devices` should contain all memory-mapped devices in slot order:
    /// device 0 is mapped to 0x100-0x1FF, device 1 to 0x200-0x2FF, etc.
    /// Typically device 0 is a LogDevice.
    pub fn new(elf: &[u8], devices: Vec<Box<dyn RamLike>>, instructions_per_update: u32) -> Self {
        let (hart, dram) = CpuBuilder::default().build(elf);
        Self {
            hart,
            dram,
            devices,
            instructions_per_update,
        }
    }

    /// Downcast a device to a concrete type.
    pub fn device_as<T: RamLike + 'static>(&self, index: usize) -> Option<&T> {
        self.devices
            .get(index)
            .and_then(|d| d.as_any().downcast_ref::<T>())
    }

    /// Downcast a device mutably to a concrete type.
    pub fn device_as_mut<T: RamLike + 'static>(&mut self, index: usize) -> Option<&mut T> {
        self.devices
            .get_mut(index)
            .and_then(|d| d.as_any_mut().downcast_mut::<T>())
    }
}

pub fn cpu_system(mut cpu_query: Query<&mut CpuComponent>) {
    for mut cpu in cpu_query.iter_mut() {
        let cpu = cpu.as_mut();
        let instructions = cpu.instructions_per_update;
        for _ in 0..instructions {
            let mut device_refs: Vec<&mut dyn RamLike> = cpu
                .devices
                .iter_mut()
                .map(|d| d.as_mut() as &mut dyn RamLike)
                .collect();

            let mut mmu = Mmu::new(&mut cpu.dram, &mut device_refs);

            // 1. Fetch.
            let inst = cpu.hart.fetch(&mmu);

            // 2. Decode.
            let (decoded, len) = Instruction::parse_with_len(inst);
            // 3. Execute.
            cpu.hart.execute(decoded, len, &mut mmu);
        }
    }
}
