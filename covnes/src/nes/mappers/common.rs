// Common utilities for all mappers to use

use std::cell::Cell;

pub enum MirrorMode {
    OneScreenLower,
    OneScreenHigher,
    Vertical,
    Horizontal,
}

pub fn get_vram_cell<'a>(
    mirror_mode: &MirrorMode,
    vram: &'a [Cell<u8>],
    addr: u16,
) -> &'a Cell<u8> {
    let addr = addr as usize;
    let offset = match addr {
        0x2000..=0x23FF => addr - 0x2000,
        0x2400..=0x27FF => addr - 0x2400,
        0x2800..=0x2BFF => addr - 0x2800,
        0x2C00..=0x2FFF => addr - 0x2C00,
        0x3000..=0x3FFF => return get_vram_cell(mirror_mode, vram, (addr - 0x1000) as u16),
        _ => panic!("Not in VRAM range"),
    };

    let base = match (addr, mirror_mode) {
        (_, MirrorMode::OneScreenLower) => 0,
        (_, MirrorMode::OneScreenHigher) => 0x400,
        (0x2000..=0x23FF, _) => 0,
        (0x2400..=0x27FF, MirrorMode::Horizontal) => 0,
        (0x2400..=0x27FF, MirrorMode::Vertical) => 0x400,
        (0x2800..=0x2BFF, MirrorMode::Horizontal) => 0x400,
        (0x2800..=0x2BFF, MirrorMode::Vertical) => 0,
        (0x2C00..=0x2FFF, _) => 0x400,
        _ => panic!("Not in VRAM range"),
    };

    &vram[base + offset]
}
