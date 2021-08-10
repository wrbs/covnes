use covnes::nes::cpu;
use covnes::nes::cpu::CpuHostAccess;
use covnes::nes::io::DummyIO;
use covnes::nes::mappers;
use covnes::nes::Nes;
use covnes::romfiles::RomFile;
use failure::Error;
use regex::Regex;
use std::fs::File;

fn do_rom(name: &str) -> Result<(), Error> {
    // Load up the rom
    let path = format!("../roms/test/{}.nes", name);
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
            panic!("Status: {:2X}\n{}", code, status)
        }
    }

    Ok(())
}

fn do_rom_instr_test_v5(name: &str) -> Result<(), Error> {
    do_rom(format!("instr_test-v5/rom_singles/{}", name).as_str())
}

#[test]
fn ppu_sprite_overflow() -> Result<(), Error> {
    do_rom("ppu_sprite_overflow")
}

#[test]
fn ppu_sprite_hit() -> Result<(), Error> {
    do_rom("ppu_sprite_hit")
}

#[test]
fn ppu_vbl_nmi() -> Result<(), Error> {
    do_rom("ppu_vbl_nmi")
}

#[test]
fn instr_test_v5() -> Result<(), Error> {
    do_rom("instr_test-v5")
}

// #[test]
fn oam_read() -> Result<(), Error> {
    do_rom("oam_read")
}

#[test]
fn m_abs_x_wrap() -> Result<(), Error> {
    do_rom_instr_test_v5("instr_misc/01-abs_x_wrap")
}

#[test]
fn m_branch_wrap() -> Result<(), Error> {
    do_rom_instr_test_v5("instr_misc/02-branch_wrap")
}

#[test]
fn m_dummy_read() -> Result<(), Error> {
    do_rom_instr_test_v5("instr_misc/03-dummy_reads")
}

/*
#[test]
fn m_dummy_read_apu() -> Result<(), Error> {
    do_rom_instr_test_v5("instr_misc/04-dummy_reads_apu")
}
*/
