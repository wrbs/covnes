use crate::system::Color;
use std::cell::Cell;
use std::process::id;
use crate::pallette;

// I got a *LOT* of help from reading https://github.com/AndreaOrru/LaiNES/blob/master/src/ppu.cpp
// in addition to (of course) NesDEV

pub struct PPU {
    // internal ram
    pub cgram: Cell<[u8; 0x20]>,
    pub oam: Cell<[u8; 0x100]>,

    // state, registers (external and internal), etc.
    pub scanline: Cell<u16>,
    pub dot: Cell<u16>,
    pub odd_frame: Cell<bool>,

    pub ppuctrl: Cell<PPUCTRL>,
    pub ppumask: Cell<PPUMASK>,
    pub ppustatus: Cell<PPUSTATUS>,
    pub oamaddr: Cell<u8>,
    pub read_buffer: Cell<u8>,
    pub last_read: Cell<u8>,

    // Scrolling related registers
    pub addr_v: Cell<u16>,
    pub addr_t: Cell<u16>,
    pub fine_x: Cell<u8>,
    pub latch_w: Cell<bool>,


    // Latches
    pub fetch_addr: Cell<u16>,
    pub fetched_nametable: Cell<u8>,
    pub fetched_attribute_table: Cell<u8>,
    pub fetched_bg_pattern_low: Cell<u8>,
    pub fetched_bg_pattern_high: Cell<u8>,
    pub at_latch_l: Cell<u8>,
    pub at_latch_h: Cell<u8>,
    // Shift regs for background
    pub bg_high_shift: Cell<u16>,
    pub bg_low_shift: Cell<u16>,
    pub at_shift_l: Cell<u8>,
    pub at_shift_h: Cell<u8>,
}

pub trait PPUHostAccess {
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&self, addr: u16, value: u8);
    fn ppu_trigger_nmi(&self);
    fn ppu_set_pixel(&self, row: u16, col: u16, color: Color);
}

bitflags! {
    pub struct PPUCTRL: u8 {
        const BASE_0 = 0x1;
        const BASE_1 = 0x2;
        const VRAM_INC = 0x4;
        const SPRITE_A = 0x8;
        const BG_TABLE_ADDRESS = 0x10;
        const SPRITE_SIZE = 0x20;
        const MASTER_SLAVE = 0x40;
        const NMI = 0x80;
    }
}

bitflags! {
    pub struct PPUMASK: u8 {
        const GREYSCALE = 0x1;
        const BG_LEFTMOST = 0x2;
        const SPRITE_LEFTMOST = 0x4;
        const SHOW_BG = 0x8;
        const SHOW_SPRITES = 0x10;
        const EMPH_RED = 0x20;
        const EMPH_GREEN = 0x40;
        const EMPH_BLUE = 0x80;
    }
}

bitflags! {
    pub struct PPUSTATUS: u8 {
        const SPRITE_OVERFLOW = 0x20;
        const ZERO_HIT = 0x40;
        const VBLANK = 0x80;
    }
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            cgram: Cell::new([0; 0x20]),
            oam: Cell::new([0; 0x100]),
            ppuctrl: Cell::new(PPUCTRL::empty()),
            ppumask: Cell::new(PPUMASK::empty()),
            ppustatus: Cell::new(PPUSTATUS::empty()),
            oamaddr: Cell::new(0),
            latch_w: Cell::new(false),
            fetch_addr: Cell::new(0),
            fetched_nametable: Cell::new(0),
            fetched_attribute_table: Cell::new(0),
            fetched_bg_pattern_low: Cell::new(0),
            fetched_bg_pattern_high: Cell::new(0),
            at_latch_l: Cell::new(0),
            at_latch_h: Cell::new(0),
            bg_high_shift: Cell::new(0),
            read_buffer: Cell::new(0),
            last_read: Cell::new(0),
            addr_v: Cell::new(0),
            addr_t: Cell::new(0),
            scanline: Cell::new(0),
            dot: Cell::new(0),
            odd_frame: Cell::new(false),
            fine_x: Cell::new(0),
            bg_low_shift: Cell::new(0),
            at_shift_l: Cell::new(0),
            at_shift_h: Cell::new(0)
        }
    }

    pub fn reset(&self) {
        self.ppuctrl.set(PPUCTRL::empty());
        self.ppumask.set(PPUMASK::empty());
        self.scanline.set(0);
        self.dot.set(0);

        // hmm - this doesn't make sense
        // see what mesen does
        self.odd_frame.set(false);
    }

    pub fn is_at_frame_end(&self) -> bool {
        self.dot.get() == 0 && self.scanline.get() == 0
    }

    pub fn is_rendering(&self) -> bool {
        let mask = self.ppumask.get();
        mask.contains(PPUMASK::SHOW_BG) || mask.contains(PPUMASK::SHOW_SPRITES)
    }

    fn cgram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.cgram;
        ram.as_slice_of_cells()
    }

    fn oam(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.oam;
        ram.as_slice_of_cells()
    }

    // https://wiki.nesdev.com/w/index.php/PPU_scrolling
    // See 'Register controls'
    pub fn reg_write<P: PPUHostAccess>(&self, host: &P, reg: u8, value: u8) {
        self.last_read.set(value);
        match reg {
            0 => {
                let old_ctrl = self.ppuctrl.get();
                let new_ctrl = PPUCTRL::from_bits_truncate(value);
                self.ppuctrl.set(new_ctrl);

                // Trigger an NMI if toggling in VBLANK without reading $2002
                if !old_ctrl.contains(PPUCTRL::NMI)
                    && new_ctrl.contains(PPUCTRL::NMI)
                    && self.ppustatus.get().contains(PPUSTATUS::VBLANK)
                {
                    host.ppu_trigger_nmi();
                }

                let t = self.addr_t.get();
                // t: ...BA.. ........ = d: ......BA
                let new_t = (t & 0b1110011_11111111) | ((value as u16 & 0b11) << 10);
                self.addr_t.set(new_t);
            }
            1 => self.ppumask.set(PPUMASK::from_bits_truncate(value)),
            3 => self.oamaddr.set(value),
            4 => {
                let oamaddr = self.oamaddr.get();
                self.oam()[oamaddr as usize].set(value);
                self.oamaddr.set(oamaddr.wrapping_add(1));
            }
            5 => {
                if self.latch_w.get() {
                    // t: CBA..HG FED..... = d: HGFEDCBA
                    let cba = (value as u16 & 0b111) << 12;
                    let hgfed = (value as u16 & !0b111) << 2;
                    let t = self.addr_t.get();
                    let new_t = (t & 0b1100_00011111) | cba | hgfed;
                    self.addr_t.set(new_t);
                    // w:                  = 0
                    self.latch_w.set(false);
                } else {
                    // t: ....... ...HGFED = d: HGFED...
                    let t = self.addr_t.get();
                    let new_t = (t & !0b11111) | (value as u16 >> 3);
                    self.addr_t.set(new_t);
                    // x:              CBA = d: .....CBA
                    self.fine_x.set(value & 0b111);
                    // w:                  = 1
                    self.latch_w.set(true);
                }
            }
            6 => {
                if self.latch_w.get() {
                    // t: ....... HGFEDCBA = d: HGFEDCBA
                    let t = self.addr_t.get();
                    let new_t = (t & !0xFF) | value as u16;
                    self.addr_t.set(new_t);
                    // v                   = t
                    self.addr_v.set(new_t);
                    // w:                  = 0
                    self.latch_w.set(false);
                } else {
                    // t: .FEDCBA ........ = d: ..FEDCBA
                    // t: X...... ........ = 0
                    let t = self.addr_t.get();
                    let new_t = (t & 0b11111111) | ((value as u16 & 0b111111) << 8);
                    self.addr_t.set(new_t);
                    // w:                  = 1
                    self.latch_w.set(true);
                }
            }
            7 => {
                let v = self.addr_v.get();
                let incr = if self.ppuctrl.get().contains(PPUCTRL::VRAM_INC) {
                    32
                } else {
                    1
                };
                self.write(host, v, value);
                self.addr_v.set((v + incr) % (1 << 15));
            }
            _ => (),
        }
    }

    pub fn reg_read<P: PPUHostAccess>(&self, host: &P, reg: u8) -> u8 {
        match reg {
            2 => {
                let mut s = self.ppustatus.get();
                let n = (self.last_read.get() & 0x1F) | s.bits();
                s.remove(PPUSTATUS::VBLANK);
                self.ppustatus.set(s);
                self.latch_w.set(false);
                self.last_read.set(n);
            }
            4 => {
                self.last_read
                    .set(self.oam()[self.oamaddr.get() as usize].get());
            }
            7 => {
                let v = self.addr_v.get() % 0x4000;
                let n = if v < 0x3F00 {
                    self.read_buffer.get()
                } else {
                    self.read(host, v)
                };

                // This is odd - normally we go through the self.read not host.read which handles
                // palette data - but the oddness is that the buffer gets the thing from the address
                // line which is normally a mirror of the nametable stuff. I'd imagine one of the
                // obscure tests I'm later going to use will test this.
                self.read_buffer.set(host.ppu_read(v));

                let incr = if self.ppuctrl.get().contains(PPUCTRL::VRAM_INC) {
                    32
                } else {
                    1
                };
                self.addr_v.set((v + incr) % (1 << 15));

                // increment addr
                self.last_read.set(n);
            }
            _ => (),
        }
        self.last_read.get()
    }

    pub fn read<P: PPUHostAccess>(&self, host: &P, addr: u16) -> u8 {
        match addr % 0x4000 {
            0x0000..=0x3EFF => host.ppu_read(addr),
            0x3F00..=0x3FFF => {
                let idx = (addr - 0x3F00) % 0x20;

                // Greyscale is done here on read
                if self.ppumask.get().contains(PPUMASK::GREYSCALE) {
                    self.cgram()[idx as usize].get() & 0x30
                } else {
                    self.cgram()[idx as usize].get()
                }
            },
            _ => panic!("Bad PPU read address"),
        }
    }

    pub fn write<P: PPUHostAccess>(&self, host: &P, addr: u16, value: u8) {
        match addr % 0x4000 {
            0x0000..=0x3EFF => host.ppu_write(addr, value),
            0x3F00..=0x3FFF => {
                let idx = (addr - 0x3F00) % 0x20;
                self.cgram()[idx as usize].set(value)
            }
            _ => panic!("Bad PPU write address"),
        }
    }

    // Outputs a pixel to the screen
    fn pixel<P: PPUHostAccess>(&self, host: &P) {
        // The docs say that the first pixel is output at dot 4
        // but sprite hit triggers at dot 2 because there's a 2 dot pipeline for palette lookup
        // However, we can probably get away with just pretending that it happens instantly I hope?

        let x = self.dot.get() as i16 - 2;

        // Check if we're in the visible
        if self.scanline.get() < 240 && x >= 0 && x < 256 {
            let x = x as u16;
            let bg_pallette = if self.ppumask.get().contains(PPUMASK::SHOW_BG) /* and side of screen mask? */ {
                let fx = self.fine_x.get();
                let pattern = ((self.bg_high_shift.get() >> (15 - fx as u16) & 1) << 1)
                    | (self.bg_low_shift.get() >> (15 - fx as u16) & 1);
                // Now we shift up to find the pallette index - we only do this if the pattern isn't
                // 0 (which means we fall back to index 0 in the pallette data)
                if pattern == 0 {
                    0
                } else {
                    // ugh
                    // The code from LaiNES explains this better as it's less rusty (in a bad way)
                    //
                    //   palette |= ((NTH_BIT(atShiftH,  7 - fX) << 1) |
                    //                NTH_BIT(atShiftL,  7 - fX))      << 2;
                    pattern | (((((self.at_shift_h.get() >> (7 - fx)) & 1) << 1 )
                        | ((self.at_shift_l.get() >> (7 - fx)) & 1 )) as u16) << 2
                }
            } else {
                0
            };

            // don't bother with sprites for now
            let fg_pallette = 0;
            let pallette_index = bg_pallette;

            let pixel_col = pallette::get_rgb(self.read(host, 0x3F00 + pallette_index));
            host.ppu_set_pixel(self.scanline.get(), x, pixel_col);
        }

        self.bg_low_shift.set(self.bg_low_shift.get() << 1);
        self.bg_high_shift.set(self.bg_high_shift.get() << 1);
        self.at_shift_l.set((self.at_shift_l.get() << 1) | self.at_latch_l.get());
        self.at_shift_h.set((self.at_shift_h.get() << 1) | self.at_latch_h.get());
    }

    pub fn tick<P: PPUHostAccess>(&self, host: &P) {
        // This section especially really has assistance from LaiNES's source code
        match self.scanline.get() {
            // Pre render and visible
            0..=239 | 261 => {
                // Clear vblank in pre-render
                if self.scanline.get() == 261 && self.dot.get() == 1{
                    // Clear vblank
                    let mut s = self.ppustatus.get();
                    s.remove(PPUSTATUS::VBLANK);
                    self.ppustatus.set(s);
                }

                // Background processing
                match self.dot.get() {
                    1 | 321 => self.fetch_addr.set(self.nt_addr()), // "NT byte 1" below without shift reloading
                    2..= 255 | 321..=337 => {
                        self.pixel(host);
                        match self.dot.get() % 8 {
                            // NT byte 1
                            1 => {
                                self.fetch_addr.set(self.nt_addr());
                                self.reload_bg_shift();
                            },
                            // NT byte 2
                            2 => self.fetched_nametable.set(self.read(host, self.fetch_addr.get())),
                            // AT byte 1
                            3 => self.fetch_addr.set(self.at_addr()),
                            // AT byte 2
                            4 => {
                                let mut at =self.read(host, self.fetch_addr.get());
                                let v = self.addr_v.get();
                                if v & 0x40 == 0x40 {  // bit 2 of coarse y
                                    at >>= 4;
                                }
                                if v & 0x2 == 0x2 {  // bit 2 of coarse x
                                    at >>= 2;
                                }
                                self.fetched_attribute_table.set(at);
                            },
                            // Low bg tile byte 1
                            5 => self.fetch_addr.set(self.bg_addr()),
                            // Low bg tile byte 2
                            6 => self.fetched_bg_pattern_low.set(self.read(host, self.fetch_addr.get())),
                            // High bg tile byte 1
                            7 => self.fetch_addr.set(self.fetch_addr.get().wrapping_add(8)),
                            // High bg tile byte 2
                            0 | _ => {
                                self.fetched_bg_pattern_high.set(self.read(host, self.fetch_addr.get()));
                                self.h_scroll();
                            },
                        }
                    },
                    256 => {
                        self.pixel(host);
                        self.fetched_bg_pattern_high.set(self.read(host, self.fetch_addr.get()));
                        self.v_scroll();
                    },
                    257 => {
                        self.pixel(host);
                        self.reload_bg_shift();
                        self.h_update();
                    },
                    280..=304 if self.scanline.get() == 261 => self.v_update(),
                    338 | 340 => { self.read(host, self.fetch_addr.get()); },
                    _ => ()
                }

                if self.scanline.get() == 261 && self.dot.get() == 339  && self.is_rendering() && self.odd_frame.get() {
                    self.dot.set(self.dot.get() + 1);
                }
            }
            // Idle for 240 (post-render)
            241 => {
                // Set vblank in 241
                if self.dot.get() == 1 {
                    // VBLANK Scanline
                    let mut s = self.ppustatus.get();
                    s.insert(PPUSTATUS::VBLANK);
                    self.ppustatus.set(s);
                    if self.ppuctrl.get().contains(PPUCTRL::NMI) {
                        host.ppu_trigger_nmi();
                    }
                }
            }
            _ => (),
        }

        let dot = self.dot.get() + 1;
        if dot > 340 {
            self.dot.set(dot % 341);
            let scanline = self.scanline.get() + 1;
            if scanline > 261 {
                self.scanline.set(scanline % 262);
                self.odd_frame.set(!self.odd_frame.get());
            } else {
                self.scanline.set(scanline);
            }
        } else {
            self.dot.set(dot);
        }
    }

    fn reload_bg_shift(&self) {
        self.bg_low_shift.set((self.bg_low_shift.get() & 0xFF00) | self.fetched_bg_pattern_low.get() as u16);
        self.bg_high_shift.set((self.bg_high_shift.get() & 0xFF00) | self.fetched_bg_pattern_high.get() as u16);

        let at = self.fetched_attribute_table.get();
        self.at_latch_l.set(at & 1);
        self.at_latch_h.set(at & 2);
    }

    fn nt_addr(&self) -> u16 {
        0x2000 | (self.addr_v.get() & 0xFFF)
    }

    fn at_addr(&self) -> u16 {
        let v = self.addr_v.get();
        0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
    }

    fn bg_addr(&self) -> u16 {
        let base = if self.ppuctrl.get().contains(PPUCTRL::BG_TABLE_ADDRESS) {
            0x1000
        } else {
            0
        };
        base + self.fetched_nametable.get() as u16 * 16 + ((self.addr_v.get() & 0x7000) >> 12)
    }

    // inc hori(v) / Course X increment
    fn h_scroll(&self) {
        if !self.is_rendering() { return }
        let mut v = self.addr_v.get();
        if (v & 0x001F) == 31 {  // if course x == 31
            v &= !0x001F;        // set course x to 0
            v ^= 0x0400;         // flip horizontal nametable
        } else {
            v += 1;
        }
        self.addr_v.set(v);
    }

    // inc vert(v)
    fn v_scroll(&self) {
        if !self.is_rendering() { return }

        let mut v = self.addr_v.get();

        if (v & 0x7000) != 0x7000 {               // If fine y < 7
            v += 0x1000;                          // Increment fine y
        } else {
            v &= !0x7000;                         // set fine y to 0
            let mut y = (v & 0x03E0) >> 5;  // let y = course y
            match y {
                29 => { y = 0; v ^= 0x0800 },     // set course y to 0, flip vert nametable
                31 => y = 0,                      // set course y to 0
                _ => y += 1,
            }
            v = (v & !0x03E0) | (y << 5)          // put course y back in to v
        }

        self.addr_v.set(v);
    }

    // hori(v) = hori(t)
    fn h_update(&self) {
        if !self.is_rendering() { return }

        let v = self.addr_v.get();
        let t = self.addr_t.get();
        self.addr_v.set((v & !0x041F) | (t & 0x41F));
    }

    // vert(v) = vert(t)
    fn v_update(&self) {
        if !self.is_rendering() { return }

        let v = self.addr_v.get();
        let t = self.addr_t.get();
        self.addr_v.set((v & !0x7BE0) | (t & 0x7BE0));
    }


}
