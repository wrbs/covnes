use crate::romfiles::{Mirroring, RomFile};
use failure::{Error, bail};
use std::cell::Cell;
use crate::nes::mappers::{Cartridge, common};
use crate::nes::mappers::common::MirrorMode;

pub fn from_rom(rom: RomFile) -> Result<Box<dyn Cartridge>, Error> {

    let banks = rom.prg_rom.len() / 16384;
    if !(banks == 1 || banks == 2 || banks == 4 || banks == 8
        || banks == 16 || banks == 32 || banks == 64 || banks == 128) {
        bail!("Badly sized prg_rom for mapper 2 (not power of 2)");
    }

    let prg_ram = if rom.provide_prg_ram {
        Some(vec![Cell::new(0); 0x2000])
    } else {
        None
    };

    let chr_data = match rom.chr_rom {
        Some(d) => {
            if d.len() != 8192 {
                bail!("Badly sized chr_rom for mapper 2")
            } else {
                Chr::ROM(d)
            }
        },
        None => Chr::RAM(vec![Cell::new(0); 8192])
    };

    let mirroring = match rom.mirroring {
        Mirroring::Horizontal => MirrorMode::Horizontal,
        Mirroring::Vertical => MirrorMode::Vertical,
        Mirroring::FourScreen => panic!("Can't do FourScreen on mapper 2/NROM"),
    };

    Ok(Box::new(UxROM {
        mirroring,
        prg_rom: rom.prg_rom,
        bank: Cell::new(0),
        chr_data,
        prg_ram,
    }))
}

enum Chr {
    ROM(Vec<u8>),
    RAM(Vec<Cell<u8>>),
}

struct UxROM {
    mirroring: common::MirrorMode,
    prg_rom: Vec<u8>,
    bank: Cell<u8>,
    chr_data: Chr,
    prg_ram: Option<Vec<Cell<u8>>>,
    // We store the PPU VRAM here in the mapper to allow for cartridges to choose
}

impl Cartridge for UxROM {
    fn read_cpu(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                if let Some(ram) = &self.prg_ram {
                    ram[(addr - 0x6000) as usize].get()
                } else {
                    if cfg!(pedantic_af) {
                        panic!("Bad read {:4X} (no PRG RAM)", addr);
                    }
                    0
                }
            }
            0x8000..=0xBFFF => {
                let addr = (addr - 0x8000) as usize;
                let base = self.bank.get() as usize * 16384;
                let addr = (base + addr) % self.prg_rom.len();
                self.prg_rom[addr]
            },
            0xC000..=0xFFFF => {
                let addr = (addr - 0xC000) as usize;
                let base = 255 * 16384; // Fix to what is always the last bank
                let addr = (base + addr) % self.prg_rom.len();
                self.prg_rom[addr]
            }
            _ => if cfg!(pedantic_af) { panic!("Bad read {:4X}", addr) } else { 0 },
        }
    }

    fn write_cpu(&self, addr: u16, value: u8) {
        match addr {
            0x6000..=0x7FFF => {
                if let Some(ram) = &self.prg_ram {
                    ram[(addr - 0x6000) as usize].set(value);
                } else {
                    if cfg!(pedantic_af) {
                        panic!("Bad write to cartridge space when no PRGRAM {:04X}", addr);
                    }
                }
            }
            0x8000..=0xFFFF =>  {
                self.bank.set(value as u8)
            },
            _ => ()
        }
    }

    fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8 {
        match addr % 0x4000 {
            0x0000..=0x1FFF => match &self.chr_data {
                Chr::ROM(r) => r[addr as usize],
                Chr::RAM(r) => r[addr as usize].get(),
            },
            0x1000..=0x3FFF => common::get_vram_cell(&self.mirroring, vram, addr).get(),
            _ => panic!("Invalid ppu read address"),
        }
    }

    fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8) {
        match addr % 0x4000 {
            0x0000..=0x1FFF => match &self.chr_data {
                Chr::ROM(_) => if cfg!(pedantic_af) { panic!("Attempt to write to CHRROM") },
                Chr::RAM(r) => r[addr as usize].set(value),
            }
            0x1000..=0x3FFF => common::get_vram_cell(&self.mirroring, vram, addr).set(value),
            _ => panic!("Invalid ppu write address"),
        }
    }
}
