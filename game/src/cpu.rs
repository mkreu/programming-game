use bevy::prelude::*;

use emulator::cpu::Cpu;

#[derive(Component)]
pub struct CpuComponent{

    cpu: Cpu
}