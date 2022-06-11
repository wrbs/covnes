use std::cell::Cell;

use crate::nes::{cpu::CpuHostAccess, io::IO, Nes};

pub struct DMA {
    pub is_odd: Cell<bool>,
    pub state: Cell<DMAState>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DMAState {
    // Not DMAing
    No,
    // Set by cpu writing to OAMDMA
    Req {
        addr_high: u8,
    },
    // If we need to for alignment
    DummyRead {
        addr_high: u8,
    },
    Read {
        addr_high: u8,
        addr_low: u8,
    },
    Write {
        addr_high: u8,
        addr_low: u8,
        value: u8,
    },
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
                    (
                        DMAState::Read {
                            addr_high,
                            addr_low: 0,
                        },
                        false,
                    )
                } else {
                    // We're currently on a read, we need to dummy read to be aligned at the end
                    (DMAState::DummyRead { addr_high }, false)
                }
            }
            DMAState::DummyRead { addr_high } => {
                nes.read((addr_high as u16) << 8);
                (
                    DMAState::Read {
                        addr_high,
                        addr_low: 0,
                    },
                    false,
                )
            }
            DMAState::Read {
                addr_high,
                addr_low,
            } => {
                let value = nes.read((addr_high as u16) << 8 | addr_low as u16);
                (
                    DMAState::Write {
                        addr_high,
                        addr_low,
                        value,
                    },
                    false,
                )
            }
            DMAState::Write {
                addr_high,
                addr_low,
                value,
            } => {
                // Write to OAMDATA
                nes.write(0x2004, value);
                if addr_low == 255 {
                    (DMAState::No, false)
                } else {
                    (
                        DMAState::Read {
                            addr_high,
                            addr_low: addr_low + 1,
                        },
                        false,
                    )
                }
            }
        };

        self.state.set(next_state);
        tick_cpu
    }
}
