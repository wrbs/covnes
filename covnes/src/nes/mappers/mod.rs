use crate::romfiles::RomFile;
use failure::{bail, Error};
use std::cell::Cell;

mod common;
mod nrom;
mod sxrom;
mod uxrom;

pub enum Cartridge {
    NotConnected,
    NROM(nrom::NROM),
    SxROM(sxrom::SxROM),
    UxROM(uxrom::UxROM),
}

pub fn from_rom(rom: RomFile) -> Result<Cartridge, Error> {
    Ok(match rom.mapper {
        0 => Cartridge::NROM(nrom::from_rom(rom)?),
        1 => Cartridge::SxROM(sxrom::from_rom(rom)?),
        2 => Cartridge::UxROM(uxrom::from_rom(rom)?),
        i => bail!("Unsupported mapper: {}", i),
    })
}

pub trait CartridgeImpl {
    fn read_cpu(&self, addr: u16) -> u8;
    fn write_cpu(&self, addr: u16, value: u8);

    fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8;
    fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8);
}

impl Cartridge {
    pub fn read_cpu(&self, addr: u16) -> u8 {
        match self {
            Cartridge::NotConnected => unimplemented!(),
            Cartridge::NROM(c) => c.read_cpu(addr),
            Cartridge::SxROM(c) => c.read_cpu(addr),
            Cartridge::UxROM(c) => c.read_cpu(addr),
        }
    }

    pub fn write_cpu(&self, addr: u16, value: u8) {
        match self {
            Cartridge::NotConnected => unimplemented!(),
            Cartridge::NROM(c) => c.write_cpu(addr, value),
            Cartridge::SxROM(c) => c.write_cpu(addr, value),
            Cartridge::UxROM(c) => c.write_cpu(addr, value),
        }
    }

    pub fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8 {
        match self {
            Cartridge::NotConnected => unimplemented!(),
            Cartridge::NROM(c) => c.read_ppu(vram, addr),
            Cartridge::SxROM(c) => c.read_ppu(vram, addr),
            Cartridge::UxROM(c) => c.read_ppu(vram, addr),
        }
    }

    pub fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8) {
        match self {
            Cartridge::NotConnected => unimplemented!(),
            Cartridge::NROM(c) => c.write_ppu(vram, addr, value),
            Cartridge::SxROM(c) => c.write_ppu(vram, addr, value),
            Cartridge::UxROM(c) => c.write_ppu(vram, addr, value),
        }
    }
}
