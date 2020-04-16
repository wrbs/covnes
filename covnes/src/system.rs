use crate::cpu::{CpuHostAccess, State, CPU};
use crate::mappers;
use crate::mappers::Cartridge;
use crate::ppu::{PPUHostAccess, PPU};
use crate::romfiles::RomFile;
use std::cell::Cell;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cycle {
    T1,
    T2,
    T3,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    const BLACK: Color = Color { r: 0, g: 0, b: 0 };
}

pub struct Nes {
    pub cpu: CPU,
    pub ppu: PPU,
    pub cartridge: Box<dyn Cartridge>,
    pub cpu_ram: Cell<[u8; 2048]>,
    pub cycle: Cell<Cycle>,
    pub pixels: Cell<[Color; 256 * 240]>,
    pub vram: Cell<[u8; 2048]>,
}

impl Nes {
    pub fn new() -> Nes {
        let cartridge = mappers::not_connected();
        let cpu = CPU::new();
        let ppu = PPU::new();
        let cpu_ram = Cell::new([0; 2048]);
        let vram = Cell::new([0; 2048]);
        let pixels = Cell::new([Color::BLACK; 256 * 240]);

        Nes {
            cpu_ram,
            ppu,
            cartridge,
            cpu,
            vram,
            pixels,
            cycle: Cell::new(Cycle::T1),
        }
    }

    pub fn reset(&self) {
        self.cpu.reset();
        self.ppu.reset()
    }

    pub fn insert_cartridge(&mut self, cartridge: Box<dyn Cartridge>) {
        self.cartridge = cartridge;
    }

    pub fn remove_cartridge(&mut self) {
        self.cartridge = mappers::not_connected();
    }

    fn ram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.cpu_ram;
        ram.as_slice_of_cells()
    }

    fn vram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.vram;
        ram.as_slice_of_cells()
    }

    fn pixels(&self) -> &[Cell<Color>] {
        let ram: &Cell<[Color]> = &self.pixels;
        ram.as_slice_of_cells()
    }

    pub fn get_pixel(&self, row: u16, col: u16) -> Color {
        self.pixels()[(row * 256 + col) as usize].get()
    }

    pub fn tick(&self) {
        let next = match self.cycle.get() {
            Cycle::T1 => {
                self.ppu.tick(self);
                // println!("{:02X} ({}, {}) {:02X}: {:?}", self.cpu.pc.get(), self.ppu.dot.get(), self.ppu.scanline.get(), self.cpu.s.get(), self.cpu.state.get());
                Cycle::T2
            }
            Cycle::T2 => {
                self.ppu.tick(self);
                Cycle::T3
            }
            Cycle::T3 => {
                self.ppu.tick(self);
                self.cpu.tick(self);
                Cycle::T1
            }
        };

        self.cycle.set(next)
    }

    pub fn tick_cpu(&self) {
        self.tick();

        while self.cycle.get() != Cycle::T1 {
            self.tick();
        }
    }

    pub fn step_cpu_instruction(&self) -> usize {
        self.tick_cpu();
        let mut ticks = 1;

        while !self.cpu.is_at_instruction() {
            self.tick_cpu();
            ticks += 1;
        }

        ticks
    }

    pub fn step_frame(&self) -> usize {
        self.tick();
        let mut ticks = 1;

        while !self.ppu.is_at_frame_end() {
            self.tick();
            ticks += 1;
        }
        // println!("{} {:?} {} {}", self.cpu.pc.get(), self.ppu.ppuctrl.get(), self.ppu.dot.get(), self.ppu.scanline.get());

        ticks
    }
}

impl CpuHostAccess for Nes {
    fn read(&self, addr: u16) -> u8 {
        let ram = self.ram();
        match addr {
            0x0000..=0x07FF => ram[addr as usize].get(),
            0x0800..=0x0FFF => ram[(addr - 0x800) as usize].get(),
            0x1000..=0x17FF => ram[(addr - 0x1000) as usize].get(),
            0x1800..=0x1FFF => ram[(addr - 0x1800) as usize].get(),
            0x2000..=0x3FFF => {
                let ppu_reg = ((addr - 0x2000) % 8) as u8;
                self.ppu.reg_read(self, ppu_reg)
            }
            0x4000..=0x4017 => {
                // println!("APU Read: 0x{:04x}", addr);
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
                let ppu_reg = ((addr - 0x2000) % 8) as u8;
                self.ppu.reg_write(self, ppu_reg, value);
            }
            0x4000..=0x4017 => {
                // println!("APU Write: 0x{:04x} {}", addr, value);
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

impl PPUHostAccess for Nes {
    fn ppu_read(&self, addr: u16) -> u8 {
        self.cartridge.read_ppu(self.vram(), addr)
    }

    fn ppu_write(&self, addr: u16, value: u8) {
        self.cartridge.write_ppu(self.vram(), addr, value)
    }

    fn ppu_trigger_nmi(&self) {
        self.cpu.set_nmi();
    }

    fn ppu_set_pixel(&self, row: u16, col: u16, color: Color) {
        self.pixels()[(row * 256 + col) as usize].set(color);
    }
}
