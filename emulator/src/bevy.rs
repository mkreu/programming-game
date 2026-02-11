use std::io::{Write, stdout};

use bevy::prelude::*;

use crate::cpu::{Dram, Hart, Instruction, LogDevice, Mmu, RamLike};

pub struct EmulatorPlugin;

impl Plugin for EmulatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (cpu_system,));
    }
}

#[derive(Component)]
pub struct CpuComponent {
    hart: Hart,
    dram: Dram,
    instructions_per_update: u32,
}

impl CpuComponent {
    fn components_mut(&mut self) -> (&mut Hart, &mut Dram) {
        (&mut self.hart, &mut self.dram)
    }
}

pub fn cpu_system(mut cpu_query: Query<&mut CpuComponent>) {
    for mut cpu in cpu_query.iter_mut() {
        for _ in 0..cpu.instructions_per_update {
            let (cpu, dram) = cpu.components_mut();
            let mut log = LogDevice;
            let mut devices : Vec<&mut dyn RamLike> = vec![&mut log];
            let mut mmu = Mmu::new(dram, &mut devices);

            // 1. Fetch.
            let inst = cpu.fetch(&mut mmu);

            // 2. Add 4 to the program counter.
            cpu.pc += 4;

            // 3. Decode.
            // 4. Execute.
            cpu.execute(Instruction::parse(inst), &mut mmu);

            let print = dram.load(4, 32).unwrap();
            dram.store(4, 32, 0).unwrap();

            if print != 0 {
                print!("{}", char::from_u32(print).unwrap());
                stdout().flush().unwrap()
            }
        }
    }
}
