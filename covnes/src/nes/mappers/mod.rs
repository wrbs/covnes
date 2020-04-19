use crate::romfiles::RomFile;
use failure::{bail, Error};
use std::cell::Cell;

pub fn not_connected() -> Box<dyn Cartridge> {
    Box::new(NotConnected)
}

mod common;
mod nrom;
mod sxrom;

pub fn from_rom(rom: RomFile) -> Result<Box<dyn Cartridge>, Error> {
    Ok(match rom.mapper {
        0 => nrom::from_rom(rom)?,
        1 => sxrom::from_rom(rom)?,
        i => bail!("Unsupported mapper: {}", rom.mapper),
    })
}

pub trait Cartridge {
    fn read_cpu(&self, addr: u16) -> u8;
    fn write_cpu(&self, addr: u16, value: u8);

    fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8;
    fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8);
}

struct NotConnected;
impl Cartridge for NotConnected {
    fn read_cpu(&self, addr: u16) -> u8 {
        unimplemented!()
    }

    fn write_cpu(&self, addr: u16, value: u8) {
        unimplemented!()
    }

    fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8 {
        unimplemented!()
    }

    fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8) {
        unimplemented!()
    }
}

