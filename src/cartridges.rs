use crate::roms::{Mirroring, RomFile};

pub fn from_rom(rom: RomFile) -> Box<dyn Cartridge> {
    let mirror_prg_rom = rom.prg_rom.len() <= 16384;

    Box::new(Mapper0 {
        mirroring: rom.mirroring,
        prg_rom: rom.prg_rom,
        chr_rom: rom.chr_rom,
        mirror_prg_rom,
    })
}

pub trait Cartridge {
    fn read_cpu(&self, addr: u16) -> u8;
    fn write_cpu(&self, addr: u16, value: u8);
}

struct Mapper0 {
    mirroring: Mirroring,
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,
    mirror_prg_rom: bool,
}

impl Cartridge for Mapper0 {
    fn read_cpu(&self, addr: u16) -> u8 {
        if self.mirror_prg_rom {
            match addr {
                0x8000..=0xBFFF => self.prg_rom[(addr - 0x8000) as usize],
                0xC000..=0xFFFF => self.prg_rom[(addr - 0xC000) as usize],
                _ => panic!("Bad read"),
            }
        } else {
            match addr {
                0x8000..=0xFFFF => self.prg_rom[(addr - 0x8000) as usize],
                _ => panic!("Bad read"),
            }
        }
    }

    fn write_cpu(&self, addr: u16, value: u8) {
        panic!("You can't write here!");
    }
}
