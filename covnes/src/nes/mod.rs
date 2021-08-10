pub mod cpu;
pub mod dma;
pub mod io;
pub mod mappers;
pub mod palette;
pub mod ppu;

use cpu::{CpuHostAccess, CPU};
use dma::DMA;
use io::IO;
use ppu::{PPUHostAccess, PPU};
use std::cell::Cell;

use self::mappers::Cartridge;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Cycle {
    T1,
    T2,
    T3,
}

pub struct Nes<I: IO> {
    pub io: I,
    pub cpu: CPU,
    pub ppu: PPU,
    pub dma: DMA,
    pub cartridge: Cartridge,
    pub cpu_ram: Cell<[u8; 2048]>,
    pub cycle: Cell<Cycle>,
    pub vram: Cell<[u8; 2048]>,
    pub controller_latch: Cell<bool>,
}

impl<I: IO> Nes<I> {
    pub fn new(io: I) -> Nes<I> {
        let cartridge = Cartridge::NotConnected;
        let cpu = CPU::new();
        let ppu = PPU::new();
        let dma = DMA::new();
        let cpu_ram = Cell::new([0; 2048]);
        let vram = Cell::new([0; 2048]);

        Nes {
            io,
            cpu_ram,
            ppu,
            dma,
            cartridge,
            cpu,
            vram,
            cycle: Cell::new(Cycle::T1),
            controller_latch: Cell::new(false),
        }
    }

    pub fn reset(&self) {
        self.cpu.reset();
        self.ppu.reset();
        self.dma.reset();
    }

    pub fn insert_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = cartridge;
    }

    pub fn remove_cartridge(&mut self) {
        self.cartridge = Cartridge::NotConnected;
    }

    fn ram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.cpu_ram;
        ram.as_slice_of_cells()
    }

    fn vram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.vram;
        ram.as_slice_of_cells()
    }

    pub fn tick(&self) {
        let next = match self.cycle.get() {
            Cycle::T1 => {
                self.perform_cpu_cycle();
                self.ppu.tick(self);
                // println!("{:02X} ({}, {}) {:02X}: {:?}", self.cpu.pc.get(), self.ppu.dot.get(), self.ppu.scanline.get(), self.cpu.s.get(), self.cpu.state.get());
                Cycle::T2
            }
            Cycle::T2 => {
                self.cpu.poll_interrupts();
                self.ppu.tick(self);
                Cycle::T3
            }
            Cycle::T3 => {
                self.ppu.tick(self);
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

    fn perform_cpu_cycle(&self) {
        let should_tick_cpu = self.dma.tick(&self);
        if should_tick_cpu {
            self.cpu.tick(self);
        }
    }
}

impl<I: IO> CpuHostAccess for Nes<I> {
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
            0x4016 => {
                // TODO open bus if I ever implement that
                self.io.controller_port_1_read().bits()
            }
            0x4017 => self.io.controller_port_2_read().bits(),
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
            0x4014 => self.dma.trigger_oamdma(value),
            0x4016 => {
                let new_l = value & 1 == 1;
                let current_l = self.controller_latch.get();
                if new_l != current_l {
                    self.controller_latch.set(new_l);
                    self.io.controller_latch_change(new_l);
                }
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

impl<I: IO> PPUHostAccess for Nes<I> {
    fn ppu_read(&self, addr: u16) -> u8 {
        self.cartridge.read_ppu(self.vram(), addr)
    }

    fn ppu_write(&self, addr: u16, value: u8) {
        self.cartridge.write_ppu(self.vram(), addr, value)
    }

    fn ppu_trigger_nmi(&self) {
        self.cpu.set_nmi();
    }

    fn ppu_suppress_nmi(&self) {
        self.cpu.clear_nmi();
    }

    fn ppu_set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.io.set_pixel(row, col, r, g, b);
    }
}
