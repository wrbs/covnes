use covnes::cpu;
use covnes::cpu::CpuHostAccess;
use covnes::romfiles::RomFile;
use covnes::system::Nes;
use covnes::io::DummyIO;
use covnes::mappers;
use failure::Error;
use regex::Regex;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::pin::Pin;

fn do_rom(name: &str) -> Result<(), Error> {
    // Load up the rom
    let path = format!("../roms/test/instr_test-v5/rom_singles/{}.nes", name);
    let mut f = File::open(path)?;
    let mut rom = RomFile::from_read(&mut f)?;
    // Hmmm, todo look into why I have to do this...
    rom.provide_prg_ram = true;

    let cart = mappers::from_rom(rom)?;

    let io = DummyIO;
    let mut nes = Nes::new(io);

    nes.insert_cartridge(cart);

    loop {
        for _ in 0..1000 {
            nes.tick_cpu();
        }
        let code = (&nes).read(0x6000);
        if code != 0 {
            break;
        }
    }

    loop {
        for _ in 0..1000 {
            nes.tick_cpu();
        }

        let mut status = String::new();
        let mut p = 0x6004;
        loop {
            let c = (&nes).read(p);
            if c == 0 {
                break;
            }

            p += 1;

            status.push(c as char);
        }

        let code = (&nes).read(0x6000);
        if code == 0 {
            break;
        } else if code != 0x80 {
            panic!("Status: {:2X} {}", code, status)
        }
    }

    Ok(())
}

#[test]
fn basics() -> Result<(), Error> {
    do_rom("01-basics")
}

#[test]
fn implied() -> Result<(), Error> {
    do_rom("02-implied")
}

#[test]
fn immediate() -> Result<(), Error> {
    do_rom("03-immediate")
}

#[test]
fn zero_page() -> Result<(), Error> {
    do_rom("04-zero_page")
}

#[test]
fn zp_xy() -> Result<(), Error> {
    do_rom("05-zp_xy")
}

#[test]
fn absolute() -> Result<(), Error> {
    do_rom("06-absolute")
}

#[test]
fn abs_xy() -> Result<(), Error> {
    do_rom("07-abs_xy")
}

#[test]
fn ind_x() -> Result<(), Error> {
    do_rom("08-ind_x")
}

#[test]
fn ind_y() -> Result<(), Error> {
    do_rom("09-ind_y")
}

#[test]
fn branches() -> Result<(), Error> {
    do_rom("10-branches")
}

#[test]
fn stack() -> Result<(), Error> {
    do_rom("11-stack")
}

#[test]
fn jmp_jsr() -> Result<(), Error> {
    do_rom("12-jmp_jsr")
}

#[test]
fn rts() -> Result<(), Error> {
    do_rom("13-rts")
}

#[test]
fn rti() -> Result<(), Error> {
    do_rom("14-rti")
}

#[test]
fn brk() -> Result<(), Error> {
    do_rom("15-brk")
}

#[test]
fn special() -> Result<(), Error> {
    do_rom("16-special")
}

#[test]
fn m_abs_x_wrap() -> Result<(), Error> {
    do_rom("instr_misc/01-abs_x_wrap")
}

#[test]
fn m_branch_wrap() -> Result<(), Error> {
    do_rom("instr_misc/02-branch_wrap")
}

#[test]
fn m_dummy_read() -> Result<(), Error> {
    do_rom("instr_misc/03-dummy_reads")
}

/*
#[test]
fn m_dummy_read_apu() -> Result<(), Error> {
    do_rom("instr_misc/04-dummy_reads_apu")
}
*/
