use crate::mappers::{Cartridge, common};
use crate::romfiles::{Mirroring, RomFile};
use failure::{Error, bail};
use std::cell::Cell;
use crate::mappers::common::MirrorMode;

pub fn from_rom(rom: RomFile) -> Result<Box<dyn Cartridge>, Error> {
    let mirror_prg_rom = rom.prg_rom.len() == 16384;

    if !(rom.prg_rom.len() == 16384 || rom.prg_rom.len() == 16384 * 2) {
        bail!("Badly sized prg_rom for mapper 0");
    }

    let prg_ram = if rom.provide_prg_ram {
        Some(Cell::new([0; 0x2000]))
    } else {
        None
    };

    let chr_data = match rom.chr_rom {
        Some(d) => {
            if d.len() != 8192 {
                bail!("Badly sized chr_rom for mapper 0")
            } else {
                ChrData::Rom(d)
            }
        },
        None => ChrData::Ram(Cell::new([0; 8192]))
    };

    let mirroring = match rom.mirroring {
        Mirroring::Horizontal => MirrorMode::Horizontal,
        Mirroring::Vertical => MirrorMode::Vertical,
        Mirroring::FourScreen => panic!("Can't do FourScreen on mapper 0/NROM"),
    };

    Ok(Box::new(NROM {
        mirroring: mirroring,
        prg_rom: rom.prg_rom,
        chr_data,
        prg_ram,
        mirror_prg_rom,
    }))
}

enum ChrData {
    Rom(Vec<u8>),
    Ram(Cell<[u8; 8192]>),
}

struct NROM {
    mirroring: common::MirrorMode,
    prg_rom: Vec<u8>,
    chr_data: ChrData,
    mirror_prg_rom: bool,
    prg_ram: Option<Cell<[u8; 0x2000]>>,
    // We store the PPU VRAM here in the mapper to allow for cartridges to choose
}

impl NROM {
    fn prg_ram(&self) -> Option<&[Cell<u8>]> {
        match &self.prg_ram {
            None => None,
            Some(r) => {
                let r: &Cell<[u8]> = r;
                Some(r.as_slice_of_cells())
            }
        }
    }
}

impl Cartridge for NROM {
    fn read_cpu(&self, addr: u16) -> u8 {
        if self.mirror_prg_rom {
            match addr {
                0x6000..=0x7FFF => {
                    if let Some(ram) = self.prg_ram() {
                        ram[(addr - 0x6000) as usize].get()
                    } else {
                        if cfg!(pedantic_af) {
                            panic!("Bad read {:4X} (no PRG RAM)", addr);
                        }
                        0
                    }
                }
                0x8000..=0xBFFF => self.prg_rom[(addr - 0x8000) as usize],
                0xC000..=0xFFFF => self.prg_rom[(addr - 0xC000) as usize],
                _ => if cfg!(pedantic_af) { panic!("Bad read") } else { 0 },
            }
        } else {
            match addr {
                0x6000..=0x7FFF => {
                    if let Some(ram) = self.prg_ram() {
                        ram[(addr - 0x6000) as usize].get()
                    } else {
                        if cfg!(pedantic_af) {
                            panic!("Bad read {:4X} (no PRG RAM)", addr);
                        }
                        0
                    }
                }
                0x8000..=0xFFFF => self.prg_rom[(addr - 0x8000) as usize],
                _ => if cfg!(pedantic_af) { panic!("Bad read {:4X}", addr) } else { 0 },
            }
        }
    }

    fn write_cpu(&self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                if let Some(ram) = self.prg_ram() {
                    ram[(addr - 0x6000) as usize].set(value);
                } else {
                    if cfg!(pedantic_af) {
                        panic!("Bad write to cartridge space when no PRGRAM {:04X}", addr);
                    }
                }
            }
            _ => if cfg!(pedantic_af) {
                panic!("Attempt to write to PRGROM {:04X}", addr)
            },
        }
    }

    fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8 {
        match addr % 0x4000 {
            0x0000..=0x1FFF => match &self.chr_data {
                ChrData::Rom(r) => r[addr as usize],
                ChrData::Ram(r) => to_sc(r)[addr as usize].get(),
            },
            0x1000..=0x3FFF => common::get_vram_cell(&self.mirroring, vram, addr).get(),
            _ => panic!("Invalid ppu read address"),
        }
    }

    fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8) {
        match addr % 0x4000 {
            0x0000..=0x1FFF => match &self.chr_data {
                ChrData::Rom(_) => if cfg!(pedantic_af) { panic!("Attempt to write to CHRROM") },
                ChrData::Ram(r) => to_sc(r)[addr as usize].set(value),
            }
            0x1000..=0x3FFF => common::get_vram_cell(&self.mirroring, vram, addr).set(value),
            _ => panic!("Invalid ppu write address"),
        }
    }
}

fn to_sc(c: &Cell<[u8]>) -> &[Cell<u8>] {
    c.as_slice_of_cells()
}
