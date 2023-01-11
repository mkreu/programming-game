use cpu::Cpu;
use std::env;
use std::{fs, io};

mod cpu;
mod dram;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        panic!("Usage: emulator <filename>");
    }
    let code = fs::read(&args[1])?;

    let mut cpu = Cpu::new(code);

    while cpu.pc < cpu.dram.len() as u32 {
        // 1. Fetch.
        let inst = cpu.fetch();

        // 2. Add 4 to the program counter.
        cpu.pc = cpu.pc + 4;

        // 3. Decode.
        // 4. Execute.
        cpu.execute(inst);
    }
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
