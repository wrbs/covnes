// This runs nestest and tests the CPU in isolation
use crate::cpu;
use crate::roms::RomFile;
use crate::system::Nes;
use failure::Error;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::pin::Pin;

#[test]
fn nestest() -> Result<(), Error> {
    // Load up the rom
    let mut f = File::open("roms/test/nestest.nes")?;
    let mut log = File::open("roms/test/nestest.log")?;
    let mut log = BufReader::new(log);

    let rom = RomFile::from_read(&mut f)?;
    let nes = Nes::from_rom(rom);

    // Setup initial state
    nes.cpu.pc.set(0xC000);
    nes.cpu.p.set(0x24);
    nes.cpu.a.set(0x0);
    nes.cpu.x.set(0x0);
    nes.cpu.y.set(0x0);
    nes.cpu.s.set(0xFD);

    let mut cycles = 7;
    let mut last_cycles = 7;

    let re = Regex::new(r"([A-F0-9]{4}).+A:([A-F0-9]{2}) X:([A-F0-9]{2}) Y:([A-F0-9]{2}) P:([A-F0-9]{2}) SP:([A-F0-9]{2}).*CYC:(\d+)").unwrap();

    loop {
        let mut buf = String::new();
        log.read_line(&mut buf)?;
        if &buf == "" {
            break;
        }

        print!("{}", &buf);

        let cap = re.captures(&buf).unwrap();
        let expected_pc = u16::from_str_radix(&cap[1], 16).unwrap();
        let expected_a = u8::from_str_radix(&cap[2], 16).unwrap();
        let expected_x = u8::from_str_radix(&cap[3], 16).unwrap();
        let expected_y = u8::from_str_radix(&cap[4], 16).unwrap();
        let expected_p = u8::from_str_radix(&cap[5], 16).unwrap();
        let expected_sp = u8::from_str_radix(&cap[6], 16).unwrap();
        let expected_cycles = usize::from_str_radix(&cap[7], 10).unwrap();

        if expected_pc != nes.cpu.pc.get()
            || expected_a != nes.cpu.a.get()
            || expected_x != nes.cpu.x.get()
            || expected_y != nes.cpu.y.get()
            || expected_sp != nes.cpu.s.get()
            || expected_p != nes.cpu.p.get()
            || expected_cycles != cycles
        {
            println!("----");
            println!("{:04X}                                            A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}             CYC:{}",
                     nes.cpu.pc.get(), nes.cpu.a.get(), nes.cpu.x.get(), nes.cpu.y.get(),
                     nes.cpu.p.get(), nes.cpu.s.get(), cycles
            );

            if expected_p != nes.cpu.p.get() {
                println!("            NV-BDIZC");
                println!("Expected P: {:08b}", expected_p);
                println!("Actual   P: {:08b}", nes.cpu.p.get());
                println!("XOR      P: {:08b}", expected_p ^ nes.cpu.p.get());
            }

            if expected_cycles != cycles {
                println!(
                    "Expected op to take {} cycles, it took {}",
                    expected_cycles - last_cycles,
                    cycles - last_cycles
                );
            }

            panic!("Bad CPU");
        }

        last_cycles = cycles;

        cycles += nes.step_cpu_instruction();
    }

    Ok(())
}
