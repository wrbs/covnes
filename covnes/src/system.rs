use crate::cpu::{CpuHostAccess, State, CPU};
use crate::mappers;
use crate::mappers::Cartridge;
use crate::ppu::{PPUHostAccess, PPU};
use crate::romfiles::RomFile;
use std::cell::Cell;

pub trait IO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8);
}

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
    pub cartridge: Box<dyn Cartridge>,
    pub cpu_ram: Cell<[u8; 2048]>,
    pub cycle: Cell<Cycle>,
    pub vram: Cell<[u8; 2048]>,
}

impl<I: IO> Nes<I> {
    pub fn new(io: I) -> Nes<I> {
        let cartridge = mappers::not_connected();
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
        }
    }

    pub fn reset(&self) {
        self.cpu.reset();
        self.ppu.reset();
        self.dma.reset();
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
                self.perform_cpu_cycle();
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

    fn ppu_set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {
        self.io.set_pixel(row, col, r, g, b);
    }
}


pub struct DMA {
    pub is_odd: Cell<bool>,
    pub state: Cell<DMAState>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DMAState {
    // Not DMAing
    No,
    // Set by cpu writing to OAMDMA
    Req { addr_high: u8 },
    // If we need to for alignment
    DummyRead { addr_high: u8 },
    Read { addr_high: u8, addr_low: u8 },
    Write { addr_high: u8, addr_low: u8, value: u8 },
}

// This is not entirely accurate - we don't read the correct address when starting a DMA
// This is because I don't want to to completely restructure the CPU just in case there happened to
// be some kind of snoopy bus
//
// Timing's there, actual reads not so much
impl DMA {
    pub fn new() -> DMA {
        DMA {
            is_odd: Cell::new(false),
            state: Cell::new(DMAState::No),
        }
    }

    pub fn reset(&self) {
        self.is_odd.set(false);
    }

    pub fn trigger_oamdma(&self, value: u8) {
        self.state.set(DMAState::Req { addr_high: value });
    }

    pub fn tick<I: IO>(&self, nes: &Nes<I>) -> bool {
        let is_odd = self.is_odd.get();
        self.is_odd.set(!is_odd);

        let (next_state, tick_cpu) = match self.state.get() {
            DMAState::No => (DMAState::No, true),
            DMAState::Req { addr_high } => {
                if nes.cpu.state.get().is_write_cycle() {
                    // We don't hijack write cycles
                    (DMAState::Req { addr_high }, true)
                } else if is_odd {
                    // This is currently a write cycle. This is good - next is read
                    (DMAState::Read { addr_high, addr_low: 0 }, false)
                } else {
                    // We're currently on a read, we need to dummy read to be aligned at the end
                    (DMAState::DummyRead { addr_high }, false)
                }
            },
            DMAState::DummyRead { addr_high } => {
                nes.read((addr_high as u16) << 8);
                (DMAState::Read { addr_high, addr_low: 0 }, false)
            },
            DMAState::Read { addr_high, addr_low } => {
                let value = nes.read ((addr_high as u16) << 8 | addr_low as u16);
                (DMAState::Write { addr_high, addr_low, value }, false)
            },
            DMAState::Write { addr_high, addr_low, value } => {
                // Write to OAMDATA
                nes.write(0x2004, value);
                if addr_low == 255 {
                    (DMAState::No, false)
                } else {
                    (DMAState::Read { addr_high, addr_low: addr_low + 1 }, false)
                }
            },
        };

        self.state.set(next_state);
        tick_cpu
    }
}

pub struct DummyIO;
impl IO for DummyIO {
    fn set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8) {}
}