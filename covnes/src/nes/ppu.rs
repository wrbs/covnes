use std::cell::Cell;

use crate::nes::palette;

// I got a *LOT* of help from reading https://github.com/AndreaOrru/LaiNES/blob/master/src/ppu.cpp
// in addition to (of course) NesDEV

pub struct PPU {
    // internal ram
    pub cgram: Cell<[u8; 32]>,
    pub oam: Cell<[u8; 0x100]>,
    // Holds 8 sprites to be rendered on the following scanline
    pub secondary_oam: Cell<[u8; 32]>,

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

    pub clear_vblank: Cell<bool>,

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

    // Sprite evalaution - help from mesen source
    pub secondary_oam_addr: Cell<u8>,
    pub oam_value_latch: Cell<u8>,
    pub sprite_in_range: Cell<bool>,
    pub sprite_evaluation_done: Cell<bool>,
    pub sprite_zero_next_scanline: Cell<bool>,

    // Sprite rendering
    pub sprites: [SpriteToRender; 8],
    pub sprite_zero_current_scanline: Cell<bool>,
    pub num_sprites: Cell<usize>,

    // Obscure timing fixes
    pub perform_skip: Cell<bool>,
}

pub trait PPUHostAccess {
    fn ppu_read(&self, addr: u16) -> u8;
    fn ppu_write(&self, addr: u16, value: u8);
    fn ppu_trigger_nmi(&self);
    fn ppu_suppress_nmi(&self);
    fn ppu_set_pixel(&self, row: u16, col: u16, r: u8, g: u8, b: u8);
}

// Contains sprite info for the current scanline
// Models the internal counters and shift registers
#[derive(Debug, Eq, PartialEq)]
pub struct SpriteToRender {
    pub x: Cell<u8>,
    pub low_pattern: Cell<u8>,
    pub high_pattern: Cell<u8>,
    pub attributes: Cell<SpriteAttributes>,
}

impl Default for SpriteToRender {
    fn default() -> Self {
        SpriteToRender {
            x: Cell::new(0),
            low_pattern: Cell::new(0),
            high_pattern: Cell::new(0),
            attributes: Cell::new(SpriteAttributes::empty()),
        }
    }
}

bitflags! {
    pub struct PPUCTRL: u8 {
        const BASE_0 = 0x1;
        const BASE_1 = 0x2;
        const VRAM_INC = 0x4;
        const SPRITE_BANK_1000 = 0x8;
        const BG_TABLE_ADDRESS = 0x10;
        const LARGE_SPRITES = 0x20;
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
        const SPRITE_0_HIT = 0x40;
        const VBLANK = 0x80;
    }
}

bitflags! {
    pub struct SpriteAttributes: u8 {
        const PALLETTE_LOW = 0x01;
        const PALLETTE_HIGH = 0x02;
        const PRIORITY_BEHIND = 0x20;
        const FLIP_HORIZ = 0x40;
        const FLIP_VERT = 0x80;
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
            clear_vblank: Cell::new(false),
            addr_v: Cell::new(0),
            addr_t: Cell::new(0),
            scanline: Cell::new(0),
            dot: Cell::new(0),
            odd_frame: Cell::new(false),
            fine_x: Cell::new(0),
            bg_low_shift: Cell::new(0),
            at_shift_l: Cell::new(0),
            at_shift_h: Cell::new(0),
            secondary_oam: Cell::new([0; 32]),
            secondary_oam_addr: Cell::new(0),
            oam_value_latch: Cell::new(0),
            sprite_in_range: Cell::new(false),
            sprite_evaluation_done: Cell::new(false),
            perform_skip: Cell::new(false),
            sprites: Default::default(),
            sprite_zero_next_scanline: Cell::new(false),
            sprite_zero_current_scanline: Cell::new(false),
            num_sprites: Cell::new(0),
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
        self.dot.get() == 1 && self.scanline.get() == 241
    }

    pub fn is_rendering(&self) -> bool {
        let mask = self.ppumask.get();
        mask.contains(PPUMASK::SHOW_BG) || mask.contains(PPUMASK::SHOW_SPRITES)
    }

    fn is_rendering_scanline(&self) -> bool {
        self.scanline.get() < 240 || self.scanline.get() == 321
    }

    pub fn cgram(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.cgram;
        ram.as_slice_of_cells()
    }

    pub fn oam(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.oam;
        ram.as_slice_of_cells()
    }

    pub fn secondary_oam(&self) -> &[Cell<u8>] {
        let ram: &Cell<[u8]> = &self.secondary_oam;
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
                    && !(self.scanline.get() == 261 && self.dot.get() == 1)
                {
                    host.ppu_trigger_nmi();
                }

                if old_ctrl.contains(PPUCTRL::NMI)
                    && !new_ctrl.contains(PPUCTRL::NMI)
                    && self.scanline.get() == 241
                    && (self.dot.get() == 2 || self.dot.get() == 3)
                {
                    host.ppu_suppress_nmi();
                }

                let t = self.addr_t.get();
                // t: ...BA.. ........ = d: ......BA
                let new_t = (t & 0b1110011_11111111) | ((value as u16 & 0b11) << 10);
                self.addr_t.set(new_t);
            }
            1 => {
                self.ppumask.set(PPUMASK::from_bits_truncate(value));
            }
            3 => self.oamaddr.set(value),
            4 => {
                if !(self.is_rendering() && self.is_rendering_scanline()) {
                    let oamaddr = self.oamaddr.get();
                    self.oam()[oamaddr as usize].set(value);
                    self.oamaddr.set(oamaddr.wrapping_add(1));
                }
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
                let s = self.ppustatus.get();
                let n = (self.last_read.get() & 0x1F) | s.bits();
                self.clear_vblank.set(true);
                if self.scanline.get() == 241 && (self.dot.get() == 2 || self.dot.get() == 3) {
                    host.ppu_suppress_nmi();
                }

                self.latch_w.set(false);
                self.last_read.set(n);
            }
            4 => {
                let addr = self.oamaddr.get();
                let v = self.oam()[addr as usize].get();
                // "The three unimplemented bits of each sprite's byte 2 do not exist in the PPU and
                // always read back as 0 on PPU revisions that allow reading PPU OAM through OAMDATA
                // ($2004). This can be emulated by ANDing byte 2 with $E3 either when writing to or
                // when reading from OAM. It has not been determined whether the PPU actually drives
                // these bits low or whether this is the effect of data bus capacitance from reading
                // the last byte of the instruction (LDA $2004, which assembles to AD 04 20)."
                let v = if addr & 0b11 == 2 { v & 0xE3 } else { v };
                self.last_read.set(v)
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
                let idx = (addr - 0x3F00) % 32;

                // Greyscale is done here on read
                if self.ppumask.get().contains(PPUMASK::GREYSCALE) {
                    self.cgram()[Self::cgram_mirror_idx(idx)].get() & 0x30
                } else {
                    self.cgram()[Self::cgram_mirror_idx(idx)].get()
                }
            }
            _ => panic!("Bad PPU read address"),
        }
    }

    pub fn write<P: PPUHostAccess>(&self, host: &P, addr: u16, value: u8) {
        match addr % 0x4000 {
            0x0000..=0x3EFF => host.ppu_write(addr, value),
            0x3F00..=0x3FFF => {
                let idx = (addr - 0x3F00) % 032;
                self.cgram()[Self::cgram_mirror_idx(idx)].set(value)
            }
            _ => panic!("Bad PPU write address"),
        }
    }

    fn cgram_mirror_idx(idx: u16) -> usize {
        // "Addresses $3F10/$3F14/$3F18/$3F1C are mirrors of $3F00/$3F04/$3F08/$3F0C. Note that this
        // goes for writing as well as reading. A symptom of not having implemented this correctly
        // in an emulator is the sky being black in Super Mario Bros., which writes the backdrop
        // color through $3F10."

        match idx {
            0x10 | 0x14 | 0x18 | 0x1C => idx as usize - 0x10,
            _ => idx as usize,
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
            let bg_palette = if self.ppumask.get().contains(PPUMASK::SHOW_BG)
                && !(!self.ppumask.get().contains(PPUMASK::BG_LEFTMOST) && x < 8)
            {
                let fx = self.fine_x.get();
                let pattern = (((self.bg_high_shift.get() >> (15 - fx as u16)) & 1) << 1)
                    | ((self.bg_low_shift.get() >> (15 - fx as u16)) & 1);
                // Now we shift up to find the palette index - we only do this if the pattern isn't
                // 0 (which means we fall back to index 0 in the palette data)
                if pattern == 0 {
                    0
                } else {
                    // The code from LaiNES explains this better as it's less rusty (in a bad way)
                    //
                    //   palette |= ((NTH_BIT(atShiftH,  7 - fX) << 1) |
                    //                NTH_BIT(atShiftL,  7 - fX))      << 2;
                    pattern
                        | (((((self.at_shift_h.get() >> (7 - fx)) & 1) << 1)
                            | ((self.at_shift_l.get() >> (7 - fx)) & 1))
                            as u16)
                            << 2
                }
            } else {
                0
            };

            let (fg_palette, priority_behind) = if self.scanline.get() >= 1
                && self.ppumask.get().contains(PPUMASK::SHOW_SPRITES)
                && !(!self.ppumask.get().contains(PPUMASK::SPRITE_LEFTMOST) && x < 8)
            {
                let mut palette = 0;
                let mut prio_behind = true;
                for i in (0..self.num_sprites.get()).rev() {
                    let sprite_x = self.sprites[i].x.get() as u16;
                    if sprite_x <= x && x < sprite_x + 8 {
                        let mut offset = (x - sprite_x) as u8;
                        let attr = self.sprites[i].attributes.get();
                        if attr.contains(SpriteAttributes::FLIP_HORIZ) {
                            offset = 7 - offset;
                        }
                        let hs = self.sprites[i].high_pattern.get();
                        let ls = self.sprites[i].low_pattern.get();
                        let sprite_palette =
                            ((hs >> (7 - offset)) & 1) << 1 | ((ls >> (7 - offset)) & 1);

                        if sprite_palette != 0 {
                            // Check for sprite zero hit
                            if self.sprite_zero_current_scanline.get()
                                && x != 255
                                && bg_palette != 0
                                && i == 0
                            {
                                let mut status = self.ppustatus.get();
                                status.insert(PPUSTATUS::SPRITE_0_HIT);
                                self.ppustatus.set(status);
                            }

                            palette = (attr.bits() & 3) << 2 | sprite_palette;
                            palette += 16;
                            prio_behind = attr.contains(SpriteAttributes::PRIORITY_BEHIND)
                        }
                    }
                }
                (palette as u16, prio_behind)
            } else {
                (0, true)
            };

            let palette_index = if fg_palette != 0 && (bg_palette == 0 || !priority_behind) {
                fg_palette
            } else {
                bg_palette
            };
            let (r, g, b) = palette::get_rgb(self.read(host, 0x3F00 + palette_index));
            host.ppu_set_pixel(self.scanline.get(), x, r, g, b);
        }

        self.bg_low_shift.set(self.bg_low_shift.get() << 1);
        self.bg_high_shift.set(self.bg_high_shift.get() << 1);
        self.at_shift_l
            .set((self.at_shift_l.get() << 1) | self.at_latch_l.get());
        self.at_shift_h
            .set((self.at_shift_h.get() << 1) | self.at_latch_h.get());
    }

    pub fn tick<P: PPUHostAccess>(&self, host: &P) {
        // Sprite evaluation and loading - only on visible scanlines
        if self.is_rendering() && self.dot.get() == 257 {
            self.num_sprites.set(0)
        }
        if self.is_rendering() && self.scanline.get() <= 239 {
            match self.dot.get() {
                1..=256 => self.perform_sprite_evaluation(),
                257..=320 => {
                    let s = self.dot.get() - 257;
                    let sprite_no = (s / 8) as usize;

                    // This isn't accurate to what gets fetched form secondary oam at each tick. I
                    // don't think that actually matters?
                    match s % 8 {
                        0 => (), // todo garbage nt byte
                        1 => (), // todo fetch garbage nt
                        2 => (), // todo garbage nt byte
                        3 => (), // todo fetch garbage nt
                        4 => {
                            // Load data from secondary OaM
                            let base = sprite_no * 4;
                            let y = self.secondary_oam()[base].get();
                            let tile_index = self.secondary_oam()[base + 1].get();
                            let attributes = SpriteAttributes::from_bits_truncate(
                                self.secondary_oam()[base + 2].get(),
                            );
                            let x = self.secondary_oam()[base + 3].get();
                            let addr = if self.get_sprite_size() == 16 {
                                let bank = if tile_index & 1 == 1 { 0x1000 } else { 0x0000 };

                                let tileno = (tile_index as u16 & !1) * 16;

                                bank + tileno
                            } else {
                                let base = if self.ppuctrl.get().contains(PPUCTRL::SPRITE_BANK_1000)
                                {
                                    0x1000
                                } else {
                                    0x0000
                                };

                                base + tile_index as u16 * 16
                            };

                            if y < 240 {
                                let mut y_offset = self.scanline.get().wrapping_sub(y as u16)
                                    % self.get_sprite_size() as u16;

                                if attributes.contains(SpriteAttributes::FLIP_VERT) {
                                    y_offset = self.get_sprite_size() as u16 - y_offset - 1;
                                }

                                if y_offset > 8 {
                                    self.fetch_addr.set(addr + 16 + (y_offset - 8));
                                } else {
                                    self.fetch_addr.set(addr + y_offset)
                                }

                                self.sprites[sprite_no].x.set(x);
                                self.sprites[sprite_no].attributes.set(attributes);

                                self.num_sprites.set(sprite_no + 1);
                            }
                        }
                        5 => {
                            let s = self.read(host, self.fetch_addr.get());
                            self.sprites[sprite_no].low_pattern.set(s);
                        }
                        6 => self.fetch_addr.set(self.fetch_addr.get() + 8),
                        7 | _ => {
                            let s = self.read(host, self.fetch_addr.get());
                            self.sprites[sprite_no].high_pattern.set(s);
                        }
                    }
                }
                321 => {
                    self.sprite_zero_current_scanline
                        .set(self.sprite_zero_next_scanline.get());
                }
                _ => (),
            }
        }

        // Actual rendering
        // This section especially really has assistance from LaiNES's source code
        match self.scanline.get() {
            // Pre render and visible
            0..=239 | 261 => {
                if self.scanline.get() == 261 && self.dot.get() == 0 {
                    // Clear overflow
                    let mut s = self.ppustatus.get();
                    s.remove(PPUSTATUS::SPRITE_OVERFLOW);
                    self.ppustatus.set(s);
                }
                if self.scanline.get() == 261 && self.dot.get() == 1 {
                    // Clear vblank
                    let mut s = self.ppustatus.get();
                    s.remove(PPUSTATUS::VBLANK | PPUSTATUS::SPRITE_0_HIT);
                    self.ppustatus.set(s);
                }

                // Background processing
                match self.dot.get() {
                    1 | 321 => self.fetch_addr.set(self.nt_addr()), // "NT byte 1" below without shift reloading
                    2..=255 | 321..=337 => {
                        self.pixel(host);
                        match self.dot.get() % 8 {
                            // NT byte 1
                            1 => {
                                self.fetch_addr.set(self.nt_addr());
                                self.reload_bg_shift();
                            }
                            // NT byte 2
                            2 => self
                                .fetched_nametable
                                .set(self.read(host, self.fetch_addr.get())),
                            // AT byte 1
                            3 => self.fetch_addr.set(self.at_addr()),
                            // AT byte 2
                            4 => {
                                let mut at = self.read(host, self.fetch_addr.get());
                                let v = self.addr_v.get();
                                if v & 0x40 == 0x40 {
                                    // bit 2 of coarse y
                                    at >>= 4;
                                }
                                if v & 0x2 == 0x2 {
                                    // bit 2 of coarse x
                                    at >>= 2;
                                }
                                self.fetched_attribute_table.set(at);
                            }
                            // Low bg tile byte 1
                            5 => self.fetch_addr.set(self.bg_addr()),
                            // Low bg tile byte 2
                            6 => self
                                .fetched_bg_pattern_low
                                .set(self.read(host, self.fetch_addr.get())),
                            // High bg tile byte 1
                            7 => self.fetch_addr.set(self.fetch_addr.get().wrapping_add(8)),
                            // High bg tile byte 2
                            0 | _ => {
                                self.fetched_bg_pattern_high
                                    .set(self.read(host, self.fetch_addr.get()));
                                self.h_scroll();
                            }
                        }
                    }
                    256 => {
                        self.pixel(host);
                        self.fetched_bg_pattern_high
                            .set(self.read(host, self.fetch_addr.get()));
                        self.v_scroll();
                    }
                    257 => {
                        self.pixel(host);
                        self.reload_bg_shift();
                        self.h_update();
                    }
                    280..=304 if self.scanline.get() == 261 => self.v_update(),
                    338 | 340 => {
                        self.read(host, self.fetch_addr.get());
                    }
                    _ => (),
                }

                match self.dot.get() {
                    257..=320 => {
                        self.oamaddr.set(0);
                    }
                    _ => (),
                }

                if self.scanline.get() == 261
                    && self.dot.get() == 338
                    && self.is_rendering()
                    && self.odd_frame.get()
                {
                    self.perform_skip.set(true)
                }
                if self.scanline.get() == 261 && self.dot.get() == 339 && self.perform_skip.get() {
                    self.dot.set(self.dot.get() + 1);
                    self.perform_skip.set(false)
                }
            }
            // Idle for 240 (post-render)
            241 => {
                // Set vblank in 241
                if self.dot.get() == 1 {
                    if !self.clear_vblank.get() {
                        // VBLANK Scanline
                        let mut s = self.ppustatus.get();
                        s.insert(PPUSTATUS::VBLANK);
                        self.ppustatus.set(s);
                        if self.ppuctrl.get().contains(PPUCTRL::NMI) {
                            host.ppu_trigger_nmi();
                        }
                    }
                }
            }
            _ => (),
        }

        // Clear the vblank
        if self.clear_vblank.get() {
            let mut s = self.ppustatus.get();
            s.remove(PPUSTATUS::VBLANK);
            self.ppustatus.set(s);
            self.clear_vblank.set(false);
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

    fn get_sprite_size(&self) -> u8 {
        if self.ppuctrl.get().contains(PPUCTRL::LARGE_SPRITES) {
            16
        } else {
            8
        }
    }

    fn reload_bg_shift(&self) {
        self.bg_low_shift
            .set((self.bg_low_shift.get() & 0xFF00) | self.fetched_bg_pattern_low.get() as u16);
        self.bg_high_shift
            .set((self.bg_high_shift.get() & 0xFF00) | self.fetched_bg_pattern_high.get() as u16);

        let at = self.fetched_attribute_table.get();
        self.at_latch_l.set(at & 1);
        self.at_latch_h.set((at & 2) >> 1);
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
        if !self.is_rendering() {
            return;
        }
        let mut v = self.addr_v.get();
        if (v & 0x001F) == 31 {
            // if course x == 31
            v &= !0x001F; // set course x to 0
            v ^= 0x0400; // flip horizontal nametable
        } else {
            v += 1;
        }
        self.addr_v.set(v);
    }

    // inc vert(v)
    fn v_scroll(&self) {
        if !self.is_rendering() {
            return;
        }

        let mut v = self.addr_v.get();

        if (v & 0x7000) != 0x7000 {
            // If fine y < 7
            v += 0x1000; // Increment fine y
        } else {
            v &= !0x7000; // set fine y to 0
            let mut y = (v & 0x03E0) >> 5; // let y = course y
            match y {
                29 => {
                    y = 0;
                    v ^= 0x0800
                } // set course y to 0, flip vert nametable
                31 => y = 0, // set course y to 0
                _ => y += 1,
            }
            v = (v & !0x03E0) | (y << 5) // put course y back in to v
        }

        self.addr_v.set(v);
    }

    // hori(v) = hori(t)
    fn h_update(&self) {
        if !self.is_rendering() {
            return;
        }

        let v = self.addr_v.get();
        let t = self.addr_t.get();
        self.addr_v.set((v & !0x041F) | (t & 0x41F));
    }

    // vert(v) = vert(t)
    fn v_update(&self) {
        if !self.is_rendering() {
            return;
        }

        let v = self.addr_v.get();
        let t = self.addr_t.get();
        self.addr_v.set((v & !0x7BE0) | (t & 0x7BE0));
    }

    fn perform_sprite_evaluation(&self) {
        // Todo - revisit this section and get the OAM reads more accurately done
        let dot = self.dot.get();
        // the OAM is internal to the PPU and we don't need to be cycle accurate with the
        // reads - so we aren't. Only do stuff on even dots
        if dot == 0 {
        } else if dot < 65 {
            self.secondary_oam()[((dot - 1) / 2) as usize].set(0xFF)
        } else if dot == 65 {
            // Let's initialise stuff here
            self.secondary_oam_addr.set(0);
            self.sprite_in_range.set(false);
            self.sprite_evaluation_done.set(false);

            self.oam_value_latch
                .set(self.oam()[self.oamaddr.get() as usize].get());
        } else if dot <= 256 {
            if dot % 2 == 1 {
                self.oam_value_latch
                    .set(self.oam()[self.oamaddr.get() as usize].get());
            } else {
                let mut secondary_oam_addr = self.secondary_oam_addr.get();
                let mut sprite_in_range = self.sprite_in_range.get();
                let mut n = (self.oamaddr.get() >> 2) & 0x3F;
                let mut m = self.oamaddr.get() & 0b11;
                let value = self.oam_value_latch.get();
                match dot {
                    66..=256 => {
                        if self.sprite_evaluation_done.get() {
                            // 4. Attempt (and fail) to copy OAM[n][0] into the next free
                            // slot in secondary OAM, and increment n (repeat until HBLANK is reached)
                            n += 1;
                        } else {
                            let scanline = self.scanline.get();
                            if !sprite_in_range
                                && scanline >= value as u16
                                && scanline < value as u16 + self.get_sprite_size() as u16
                            {
                                sprite_in_range = true;
                            }

                            if dot == 66 {
                                self.sprite_zero_next_scanline.set(sprite_in_range);
                            }

                            if secondary_oam_addr < 0x20 {
                                self.secondary_oam()[secondary_oam_addr as usize].set(value);

                                if sprite_in_range {
                                    // 1a. If Y-coordinate is in range, copy remaining bytes of sprite
                                    // data (OAM[n][1] thru OAM[n][3]) into secondary OAM.
                                    m += 1;
                                    secondary_oam_addr += 1;

                                    if m == 4 {
                                        // We're done
                                        sprite_in_range = false;
                                        m = 0;
                                        n = (n + 1) % 64;

                                        // 2a. If n has overflowed back to zero (all 64 sprites
                                        // evaluated), go to 4
                                        if n == 0 {
                                            self.sprite_evaluation_done.set(true);
                                        }
                                    }
                                } else {
                                    // if sprite not in range
                                    // Go up a sprite

                                    n = (n + 1) % 64;
                                    if n == 0 {
                                        self.sprite_evaluation_done.set(true);
                                    }
                                }
                            } else {
                                // if secondary oam full
                                if sprite_in_range {
                                    // 3a. If the value is in range, set the sprite overflow flag in
                                    // $2002 and read the next 3 entries of OAM (incrementing 'm' after
                                    // each byte and incrementing 'n' when 'm' overflows);
                                    // if m = 3, increment n

                                    // Overflow detected!
                                    let mut status = self.ppustatus.get();
                                    status.insert(PPUSTATUS::SPRITE_OVERFLOW);
                                    self.ppustatus.set(status);
                                    self.sprite_evaluation_done.set(true);
                                    // We should read 3 more times but eh we can't all be mesen level yet
                                } else {
                                    // 3b. If the value is not in range, increment n and m (without
                                    // carry). If n overflows to 0, go to 4; otherwise go to 3
                                    //
                                    // The m increment is a hardware bug - if only n was incremented,
                                    // the overflow flag would be set whenever more than 8 sprites were
                                    // present on the same scanline, as expected.
                                    n = (n + 1) % 64;
                                    if n == 0 {
                                        self.sprite_evaluation_done.set(true);
                                    }

                                    m = (m + 1) % 4;
                                }
                            }
                        }
                    }
                    _ => (),
                }

                self.sprite_in_range.set(sprite_in_range);
                self.secondary_oam_addr.set(secondary_oam_addr);
                self.oamaddr.set(n << 2 | m);
            }
        }
    }
}
