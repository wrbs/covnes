use std::cell::Cell;

use anyhow::Result;

use crate::{
    nes::mappers::{common, common::MirrorMode, CartridgeImpl},
    romfiles::RomFile,
};

const LOAD_REG_INITIAL: u8 = 0b10000;

pub fn from_rom(rom: RomFile) -> Result<SxROM> {
    // This is a hack for the certain values I need to get the combined instr_test-v5 rom working
    // Basically just SNROM with 256 prg rom, prg ram, 8kb chr ram not rom

    // when (/if?) I get to the point of doing other sxrom games I can do all the special casing
    // on the high address lines

    // These assertions are false in general
    println!(
        "{} {:?}",
        rom.prg_rom.len(),
        rom.chr_rom.as_ref().map(|x| x.len())
    );
    let prg_banks = rom.prg_rom.len() / 16384;
    assert!(
        prg_banks == 2 || prg_banks == 4 || prg_banks == 8 || prg_banks == 16 || prg_banks == 32
    );
    if let Some(r) = &rom.chr_rom {
        let rom_banks = r.len() / 8192;
        assert!(
            rom_banks == 1 || rom_banks == 2 || rom_banks == 4 || rom_banks == 8 || rom_banks == 16
        );
    }

    let chr = match rom.chr_rom {
        None => ChrData::RAM(vec![Cell::new(0); 0x2000]),
        Some(r) => ChrData::ROM(r),
    };

    let prg_ram = if rom.provide_prg_ram {
        Some(vec![Cell::new(0); 0x2000])
    } else {
        None
    };

    Ok(SxROM {
        prg_rom: rom.prg_rom,
        prg_ram,
        chr,
        load_reg: Cell::new(LOAD_REG_INITIAL),
        control: Cell::new(0b01100),
        chr_bank_0: Cell::new(0),
        chr_bank_1: Cell::new(0),
        prg_bank: Cell::new(0),
    })
}

pub struct SxROM {
    prg_rom: Vec<u8>,
    chr: ChrData,
    prg_ram: Option<Vec<Cell<u8>>>,
    // Registers
    load_reg: Cell<u8>,
    control: Cell<u8>,
    chr_bank_0: Cell<u8>,
    chr_bank_1: Cell<u8>,
    prg_bank: Cell<u8>,
}

enum ChrData {
    ROM(Vec<u8>),
    RAM(Vec<Cell<u8>>),
}

impl SxROM {
    fn get_mirroring(&self) -> MirrorMode {
        match self.control.get() & 0b11 {
            0 => MirrorMode::OneScreenLower,
            1 => MirrorMode::OneScreenHigher,
            2 => MirrorMode::Vertical,
            3 | _ => MirrorMode::Horizontal,
        }
    }

    fn get_mapped_chr_addr(&self, addr: u16) -> usize {
        let chr_size = match &self.chr {
            ChrData::ROM(r) => r.len(),
            ChrData::RAM(r) => r.len(),
        };

        // Because we're only doing 8kb we can simplify this logic significantly
        if self.control.get() & 0x10 == 0x10 {
            // Two separate 4kb bytes
            if addr < 0x1000 {
                (self.chr_bank_0.get() as usize * 0x1000) % chr_size + (addr as usize)
            } else {
                (self.chr_bank_1.get() as usize * 0x1000) % chr_size + (addr as usize - 0x1000)
            }
        } else {
            ((self.chr_bank_0.get() as usize & !1) * 0x2000) % chr_size + addr as usize
        }
    }
}

impl CartridgeImpl for SxROM {
    fn read_cpu(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x5FFF => {
                panic!("Bad cpu read to cartridge: {:04X}", addr)
            }
            0x6000..=0x7FFF => match &self.prg_ram {
                None => 0x0,
                Some(r) => r[(addr - 0x6000) as usize].get(),
            },
            0x8000..=0xFFFF => {
                let control_h = self.control.get() & 8 == 8;
                let control_l = self.control.get() & 4 == 4;
                let bank = self.prg_bank.get();
                let (bank, offset) = if control_h && control_l {
                    // Fix last bank, switch other
                    if addr < 0xC000 {
                        (bank, addr - 0x8000)
                    } else {
                        (31, addr - 0xC000)
                    }
                } else if control_h && !control_l {
                    // Fix first bank, switch other
                    if addr < 0xC000 {
                        (0, addr - 0x8000)
                    } else {
                        (bank, addr - 0xC000)
                    }
                } else {
                    // Switch 32kb, ignoring low bit of bank
                    (bank & 0b11110, addr - 0x8000)
                };

                let addr = ((bank as usize) << 14) | (offset as usize);
                let index = addr % self.prg_rom.len();
                self.prg_rom[index]
            }
        }
    }

    fn write_cpu(&self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x5FFF => {
                panic!("Bad cpu write to cartridge: {:04X}", addr);
            }
            0x6000..=0x7FFF => match &self.prg_ram {
                None => (),
                Some(r) => r[(addr - 0x6000) as usize].set(value),
            },
            0x8000..=0xFFFF => {
                if value & 0x80 == 0x80 {
                    self.load_reg.set(LOAD_REG_INITIAL);
                } else {
                    // Shift in
                    let old_load_reg = self.load_reg.get();
                    let new_load_reg = (old_load_reg >> 1) | ((value & 1) << 4);

                    if old_load_reg & 1 == 1 {
                        // Reached the end
                        self.load_reg.set(LOAD_REG_INITIAL);
                        match addr {
                            0x8000..=0x9FFF => self.control.set(new_load_reg),
                            0xA000..=0xBFFF => self.chr_bank_0.set(new_load_reg),
                            0xC000..=0xDFFF => self.chr_bank_1.set(new_load_reg),
                            0xE000..=0xFFFF => self.prg_bank.set(new_load_reg),
                            _ => panic!("Unreachable"),
                        }
                    } else {
                        self.load_reg.set(new_load_reg);
                    }
                }
            }
        }
    }

    fn read_ppu(&self, vram: &[Cell<u8>], addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => match &self.chr {
                ChrData::ROM(r) => r[self.get_mapped_chr_addr(addr)],
                ChrData::RAM(r) => r[self.get_mapped_chr_addr(addr)].get(),
            },
            0x1000..=0x3FFF => common::get_vram_cell(&self.get_mirroring(), vram, addr).get(),
            _ => panic!("Invalid ppu read address"),
        }
    }

    fn write_ppu(&self, vram: &[Cell<u8>], addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => match &self.chr {
                ChrData::ROM(_) => (),
                ChrData::RAM(r) => r[self.get_mapped_chr_addr(addr)].set(value),
            },
            0x1000..=0x3FFF => common::get_vram_cell(&self.get_mirroring(), vram, addr).set(value),
            _ => panic!("Invalid ppu write address"),
        }
    }
}
