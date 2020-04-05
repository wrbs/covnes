use crate::cartridges;
use crate::cartridges::Cartridge;
use crate::cpu::{Cpu, CpuHostAccess, State};
use crate::roms::RomFile;
use std::cell::Cell;

pub struct Nes {
    pub cpu: Cpu,
    pub cartridge: Box<dyn Cartridge>,
    pub cpu_ram: Cell<[u8; 2048]>,
}

impl Nes {
    pub fn from_rom(romfile: RomFile) -> Nes {
        let cartridge = cartridges::from_rom(romfile);
        let cpu = Cpu {
            pc: Cell::new(0xC000),
            s: Cell::new(0),
            p: Cell::new(0),
            a: Cell::new(0),
            x: Cell::new(0),
            y: Cell::new(0),
            state: Cell::new(State::default())
        };

        let cpu_ram = Cell::new([0; 2048]);

        Nes {
            cpu_ram,
            cartridge,
            cpu,
        }
    }

    fn ram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.cpu_ram;
        ram.as_slice_of_cells()
    }

    pub fn step_cpu_instruction(&self) -> usize {
        self.cpu.tick(&self);
        let mut ticks = 1;

        while !self.cpu.is_at_instruction() {
            self.cpu.tick(&self);
            ticks += 1;
        }

        ticks
    }
}

impl CpuHostAccess for &Nes {
    fn read(&self, addr: u16) -> u8 {
        let ram = self.ram();
        match addr {
            0x0000..=0x07FF => ram[addr as usize].get(),
            0x0800..=0x0FFF => ram[(addr - 0x800) as usize].get(),
            0x1000..=0x17FF => ram[(addr - 0x1000) as usize].get(),
            0x1800..=0x1FFF => ram[(addr - 0x1800) as usize].get(),
            0x2000..=0x3FFF => {
                let ppu_reg = ((addr - 0x2000) % 8) as usize;
                println!("PPU Read: {}", ppu_reg);
                0
            }
            0x4000..=0x4017 => {
                println!("APU Read: 0x{:04x}", addr);
                0
            }
            0x4018..=0x401F => {
                panic!("Read from CPU test stuff");
            }
            0x4020..=0xFFFF => self.cartridge.read_cpu(addr),
        }
    }

    fn write(&self, addr: u16, value: u8) {
        let ram = self.ram();
        match addr {
            0x0000..=0x07FF => ram[addr as usize].set(value),
            0x0800..=0x0FFF => ram[(addr - 0x800) as usize].set(value),
            0x1000..=0x17FF => ram[(addr - 0x1000) as usize].set(value),
            0x1800..=0x1FFF => ram[(addr - 0x1800) as usize].set(value),
            0x2000..=0x3FFF => {
                let ppu_reg = ((addr - 0x2000) % 8) as usize;
                println!("PPU Write: {} {}", ppu_reg, value)
            }
            0x4000..=0x4017 => {
                println!("APU Read: 0x{:04x} {}", addr, value);
            }
            0x4018..=0x401F => {
                panic!("Write to CPU test stuff");
            }
            0x4020..=0xFFFF => {
                self.cartridge.write_cpu(addr, value);
            }
        }
    }
}
