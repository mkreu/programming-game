use cpu::Cpu;
use dram::{Dram, DRAM_SIZE};
use std::env;
use std::{fs, io};
use tracing::metadata::LevelFilter;
use tracing::{debug, info};
use color_eyre::Result;

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

    let mut cpu = Cpu::new(dram, entry);

    tui::run(cpu)
}

fn run_cpu() -> io::Result<()> {
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        panic!("Usage: emulator <filename>");
    }
    let code = fs::read(&args[1])?;

    let (mut dram, entry) = Dram::new(code);

    dram.store(DRAM_SIZE - 4, 32, 4).unwrap();

    let mut cpu = Cpu::new(dram, entry);

    info!("{}", cpu.dram.dram.len());
    info!("{:x}", cpu.dram.dram.len());

    while cpu.pc < cpu.dram.dram.len() as u32 {
        // 1. Fetch.
        let inst = cpu.fetch();

        // 2. Add 4 to the program counter.
        cpu.pc = cpu.pc + 4;

        // 3. Decode.
        // 4. Execute.
        cpu.execute(inst);

        info!("{}", cpu.dram.load(0x400, 32).unwrap());
        //debug!("sp: {:x}", cpu.regs[2]);
        //debug!("ra: {:x}", cpu.regs[1]);
    }
    debug!("exited loop ?? {:x}", cpu.pc);
    Ok(())
}

//pub fn main() {
//    let x: u32 = 0xfff00000;
//    let x2 = x >> 12;
//    let y = x as i32;
//    let y2 = y >> 12;
//    println!("{x}");
//    println!("{x2}");
//    println!("{y}");
//    println!("{y2}");
//}
