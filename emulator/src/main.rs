use color_eyre::Result;
use cpu::instruction::Instruction;
use cpu::Cpu;
use dram::{Dram, DRAM_SIZE};
use std::env;
use std::fs;

mod cpu;
mod dram;
mod tui;
fn main() -> Result<()> {
    //tracing_subscriber::FmtSubscriber::builder()
    //    .with_max_level(LevelFilter::DEBUG)
    //    .init();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        panic!("Usage: emulator <filename>");
    }
    let code = fs::read(&args[1])?;

    let (mut dram, entry) = Dram::new(code);

    dram.store(DRAM_SIZE - 4, 32, 4).unwrap();

    let cpu = Cpu::new(dram, entry);

    run_plain(cpu);

    //tui::run(cpu)
    Ok(())
}

fn run_plain(mut cpu: Cpu) {
    loop {
        // 1. Fetch.
        let inst = cpu.fetch();

        // 2. Add 4 to the program counter.
        cpu.pc = cpu.pc + 4;

        // 3. Decode.
        // 4. Execute.
        cpu.execute(Instruction::parse(inst));

        let print = cpu.dram.load(4, 32).unwrap();
        cpu.dram.store(4, 32, 0).unwrap();

        if print != 0 {
            print!("{}", char::from_u32(print).unwrap());
        }
    }
}
