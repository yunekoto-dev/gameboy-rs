use crate::{SCREEN_HEIGHT, SCREEN_WIDTH};
use super::interrupt::{Interrupt, Interrupts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { HBlank=0, VBlank=1, Oam=2, Transfer=3 }

pub struct Ppu {
    cgb: bool,
    vram: [u8; 0x4000],
    vram_bank: u8,
    oam: [u8; 0xA0],
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    bg_palette: [u8; 0x40],
    obj_palette: [u8; 0x40],
    bgpi: u8,
    obpi: u8,
    opri: u8,
    mode: Mode,
    dot: u16,
    pub frame: [u32; SCREEN_WIDTH * SCREEN_HEIGHT],
    pub frame_ready: bool,
    window_line: u8,
    stat_signal: bool,
    pub hblank_started: bool,
    mode3_len: u16,
    bg_color_index: [u8; SCREEN_WIDTH],
    bg_priority: [bool; SCREEN_WIDTH],
}

impl Ppu {
    pub fn new(cgb: bool) -> Self {
        let mut p = Self {
            cgb,
            vram: [0; 0x4000],
            vram_bank: 0,
            oam: [0; 0xA0],
            lcdc: 0x91,
            stat: 0x85,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            bg_palette: [0; 0x40],
            obj_palette: [0; 0x40],
            bgpi: 0,
            obpi: 0,
            opri: 0,
            mode: Mode::Oam,
            dot: 0,
            frame: [0; SCREEN_WIDTH * SCREEN_HEIGHT],
            frame_ready: false,
            window_line: 0,
            stat_signal: false,
            hblank_started: false,
            mode3_len: 172,
            bg_color_index: [0; SCREEN_WIDTH],
            bg_priority: [false; SCREEN_WIDTH],
        };
        // Pleasant default CGB palette: four grayscale entries per palette.
        const DEFAULTS: [u16; 4] = [0x7FFF, 0x56B5, 0x2D6B, 0x0000];
        for pal in 0..8usize {
            for (color, value) in DEFAULTS.iter().enumerate() {
                let raw = value.to_le_bytes();
                let base = pal * 8 + color * 2;
                p.bg_palette[base] = raw[0];
                p.bg_palette[base + 1] = raw[1];
                p.obj_palette[base] = raw[0];
                p.obj_palette[base + 1] = raw[1];
            }
        }
        p.update_stat_signal(&mut Interrupts::new());
        p
    }

    pub fn is_cgb(&self) -> bool { self.cgb }
    pub fn vram_bank(&self) -> u8 { self.vram_bank }
    pub fn set_vram_bank(&mut self, value: u8) { self.vram_bank = value & 1; }

    pub fn read_vram(&self, addr: u16) -> u8 {
        let index = (self.vram_bank as usize) * 0x2000 + (addr as usize & 0x1FFF);
        self.vram[index]
    }

    pub fn read_vram_bank(&self, bank: u8, addr: u16) -> u8 {
        let index = ((bank & 1) as usize) * 0x2000 + (addr as usize & 0x1FFF);
        self.vram[index]
    }

    pub fn write_vram(&mut self, addr: u16, value: u8) {
        let index = (self.vram_bank as usize) * 0x2000 + (addr as usize & 0x1FFF);
        self.vram[index] = value;
    }

    pub fn write_vram_bank(&mut self, bank: u8, addr: u16, value: u8) {
        let index = ((bank & 1) as usize) * 0x2000 + (addr as usize & 0x1FFF);
        self.vram[index] = value;
    }

    pub fn cpu_read_vram(&self, addr: u16) -> u8 {
        if self.lcd_enabled() && self.mode == Mode::Transfer { 0xFF }
        else { self.read_vram(addr) }
    }

    pub fn cpu_write_vram(&mut self, addr: u16, value: u8) {
        if !self.lcd_enabled() || self.mode != Mode::Transfer {
            self.write_vram(addr, value);
        }
    }

    pub fn read_oam(&self, addr: u16) -> u8 {
        self.oam[(addr as usize) % self.oam.len()]
    }

    pub fn write_oam(&mut self, addr: u16, value: u8) {
        self.oam[(addr as usize) % self.oam.len()] = value;
    }

    pub fn cpu_read_oam(&self, addr: u16) -> u8 {
        if self.lcd_enabled() && matches!(self.mode, Mode::Oam | Mode::Transfer) { 0xFF }
        else { self.read_oam(addr) }
    }

    pub fn cpu_write_oam(&mut self, addr: u16, value: u8) {
        if !self.lcd_enabled() || matches!(self.mode, Mode::HBlank | Mode::VBlank) {
            self.write_oam(addr, value);
        }
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => (self.stat & 0x78) | 0x80 | self.mode_bits() | if self.ly == self.lyc { 0x04 } else { 0 },
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            0xFF4F => 0xFE | self.vram_bank,
            0xFF68 => self.bgpi | 0x40,
            0xFF69 => if self.lcd_enabled() && self.mode == Mode::Transfer { 0xFF } else { self.palette_read(&self.bg_palette, self.bgpi) },
            0xFF6A => self.obpi | 0x40,
            0xFF6B => if self.lcd_enabled() && self.mode == Mode::Transfer { 0xFF } else { self.palette_read(&self.obj_palette, self.obpi) },
            0xFF6C => self.opri | 0xFE,
            _ => 0xFF,
        }
    }

    pub fn write_reg(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF40 => {
                let was_on = self.lcd_enabled();
                self.lcdc = value;
                if !was_on && self.lcd_enabled() {
                    self.mode = Mode::Oam;
                    self.dot = 0;
                    self.ly = 0;
                }
                if was_on && !self.lcd_enabled() {
                    self.mode = Mode::HBlank;
                    self.dot = 0;
                    self.ly = 0;
                    self.window_line = 0;
                }
            }
            0xFF41 => self.stat = value & 0x78,
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => {},
            0xFF45 => self.lyc = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            0xFF4F => self.set_vram_bank(value),
            0xFF68 => self.bgpi = value & 0xBF,
            0xFF69 => if !self.lcd_enabled() || self.mode != Mode::Transfer { self.palette_write(true, value) },
            0xFF6A => self.obpi = value & 0xBF,
            0xFF6B => if !self.lcd_enabled() || self.mode != Mode::Transfer { self.palette_write(false, value) },
            0xFF6C => self.opri = value & 1,
            _ => {}
        }
    }

    pub fn lcd_enabled(&self) -> bool { self.lcdc & 0x80 != 0 }
    pub fn mode(&self) -> Mode { self.mode }

    pub fn tick(&mut self, irq: &mut Interrupts) {
        self.hblank_started = false;
        if !self.lcd_enabled() {
            self.mode = Mode::HBlank;
            self.dot = 0;
            self.ly = 0;
            return;
        }

        self.dot = self.dot.wrapping_add(1);
        let old_mode = self.mode;
        let old_ly = self.ly;

        if self.ly < 144 {
            if self.dot < 80 {
                self.mode = Mode::Oam;
            } else if self.dot < 80 + self.mode3_len {
                self.mode = Mode::Transfer;
            } else {
                self.mode = Mode::HBlank;
            }
        } else {
            self.mode = Mode::VBlank;
        }

        if self.dot == 80 && self.ly < 144 {
            self.mode3_len = self.estimate_mode3_length();
            self.render_scanline();
        }
        if old_mode != Mode::HBlank && self.mode == Mode::HBlank && self.ly < 144 {
            self.hblank_started = true;
        }

        if self.dot >= 456 {
            self.dot = 0;
            self.ly = self.ly.wrapping_add(1);
            if self.ly == 144 {
                self.mode = Mode::VBlank;
                self.frame_ready = true;
                irq.request(Interrupt::VBlank);
                self.window_line = 0;
            } else if self.ly > 153 {
                self.ly = 0;
                self.mode = Mode::Oam;
            }
        }

        let _ = (old_mode, old_ly);
        self.update_stat_signal(irq);
    }

    fn mode_bits(&self) -> u8 { self.mode as u8 }

    fn update_stat_signal(&mut self, irq: &mut Interrupts) {
        let coincidence = self.ly == self.lyc;
        let cond = ((self.stat & 0x40 != 0) && coincidence)
            || ((self.stat & 0x20 != 0) && self.mode == Mode::Oam)
            || ((self.stat & 0x10 != 0) && self.mode == Mode::VBlank)
            || ((self.stat & 0x08 != 0) && self.mode == Mode::HBlank);
        if cond && !self.stat_signal { irq.request(Interrupt::Stat); }
        self.stat_signal = cond;
    }

    fn render_scanline(&mut self) {
        let y = self.ly as usize;
        if y >= SCREEN_HEIGHT { return; }

        let bg_enabled = self.cgb || (self.lcdc & 0x01 != 0);
        let bg_master_priority = self.lcdc & 0x01 != 0;
        let window_enabled = self.lcdc & 0x20 != 0;
        let sprite_enabled = self.lcdc & 0x02 != 0;
        let sprite_height = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let mut window_used = false;

        for x in 0..SCREEN_WIDTH {
            let use_window = window_enabled
                && self.ly >= self.wy
                && self.wx <= 166
                && (x as i16) + 7 >= self.wx as i16;

            let (color, priority, rgb) = if bg_enabled {
                let (tx, ty, map_base) = if use_window {
                    window_used = true;
                    let wx = self.wx.saturating_sub(7) as usize;
                    (x.saturating_sub(wx), self.window_line as usize,
                     if self.lcdc & 0x40 != 0 { 0x1C00 } else { 0x1800 })
                } else {
                    let px = x + self.scx as usize;
                    let py = y + self.scy as usize;
                    (px, py, if self.lcdc & 0x08 != 0 { 0x1C00 } else { 0x1800 })
                };
                let tile_x = (tx >> 3) & 31;
                let tile_y = (ty >> 3) & 31;
                let map_addr = map_base + tile_y * 32 + tile_x;
                let tile_id = self.read_vram_bank(0, map_addr as u16);
                let attr = if self.cgb { self.read_vram_bank(1, map_addr as u16) } else { 0 };
                let bank = if self.cgb { (attr >> 3) & 1 } else { 0 };
                let flip_x = self.cgb && attr & 0x20 != 0;
                let flip_y = self.cgb && attr & 0x40 != 0;
                let mut row = ty & 7;
                if flip_y { row = 7 - row; }
                let signed_mode = self.lcdc & 0x10 == 0;
                let tile_addr = if signed_mode {
                    0x1000isize + (tile_id as i8 as isize) * 16
                } else {
                    tile_id as isize * 16
                };
                let addr = (tile_addr + (row * 2) as isize) as usize & 0x1FFF;
                let lo = self.read_vram_bank(bank, addr as u16);
                let hi = self.read_vram_bank(bank, (addr + 1) as u16);
                let bit = if flip_x { tx & 7 } else { 7 - (tx & 7) };
                let color = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
                let priority = self.cgb && attr & 0x80 != 0;
                let rgb = if self.cgb {
                    let pal = attr & 0x07;
                    self.cgb_palette_color(true, pal, color)
                } else {
                    dmg_shade(self.bgp, color)
                };
                (color, priority, rgb)
            } else {
                let color = 0u8;
                let priority = false;
                let rgb = if self.cgb { self.cgb_palette_color(true, 0, 0) } else { dmg_shade(self.bgp, 0) };
                (color, priority, rgb)
            };

            self.bg_color_index[x] = color;
            self.bg_priority[x] = priority;
            self.frame[y * SCREEN_WIDTH + x] = rgb;
        }

        if window_used { self.window_line = self.window_line.wrapping_add(1); }
        if !sprite_enabled { return; }

        let mut sprites: Vec<(i16, i16, u8, u8, usize)> = Vec::with_capacity(10);
        for i in 0..40usize {
            let base = i * 4;
            let sy = self.oam[base] as i16 - 16;
            let sx = self.oam[base + 1] as i16 - 8;
            if (self.ly as i16) >= sy && (self.ly as i16) < sy + sprite_height {
                sprites.push((sx, sy, self.oam[base + 2], self.oam[base + 3], i));
                if sprites.len() == 10 { break; }
            }
        }

        // CGB mode normally uses OAM order. OPRI=1 selects DMG-style X priority.
        if self.cgb && self.opri == 0 {
            sprites.sort_by_key(|s| s.4);
        } else {
            sprites.sort_by_key(|s| (s.0, s.4));
        }

        let mut obj_selected = [false; SCREEN_WIDTH];
        for &(sx, sy, mut tile, attr, _) in sprites.iter() {
            let flip_x = attr & 0x20 != 0;
            let flip_y = attr & 0x40 != 0;
            let bank = if self.cgb { (attr >> 3) & 1 } else { 0 };
            let dm_palette = if attr & 0x10 != 0 { self.obp1 } else { self.obp0 };
            let cgb_palette = attr & 0x07;
            let mut row = self.ly as i16 - sy;
            if flip_y { row = sprite_height - 1 - row; }
            if sprite_height == 16 {
                tile &= 0xFE;
                if row >= 8 { tile = tile.wrapping_add(1); row -= 8; }
            }
            let addr = tile as usize * 16 + row as usize * 2;
            let lo = self.read_vram_bank(bank, addr as u16);
            let hi = self.read_vram_bank(bank, (addr + 1) as u16);

            for px in 0..8i16 {
                let screen_x = sx + px;
                if !(0..SCREEN_WIDTH as i16).contains(&screen_x) { continue; }
                let bit = if flip_x { px } else { 7 - px };
                let c = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
                if c == 0 { continue; }
                let x = screen_x as usize;

                // Resolve OBJ-vs-OBJ priority first; BG priority is evaluated only after
                // the winning object pixel has been selected.
                if obj_selected[x] { continue; }
                obj_selected[x] = true;

                let bg_nonzero = self.bg_color_index[x] != 0;
                let bg_blocks_obj = bg_master_priority && bg_nonzero && (self.bg_priority[x] || attr & 0x80 != 0);
                if bg_blocks_obj { continue; }

                self.frame[y * SCREEN_WIDTH + x] = if self.cgb {
                    self.cgb_palette_color(false, cgb_palette, c)
                } else {
                    dmg_shade(dm_palette, c)
                };
            }
        }
    }

    fn estimate_mode3_length(&self) -> u16 {
        // Mode 3 is not fixed on real hardware. This model accounts for the major
        // observable sources of extra work (visible objects and window startup) while
        // remaining deterministic. The exact per-dot FIFO implementation is a future step.
        let sprite_height = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let mut sprites = 0u16;
        if self.lcdc & 0x02 != 0 {
            for i in 0..40usize {
                let base = i * 4;
                let sy = self.oam[base] as i16 - 16;
                if (self.ly as i16) >= sy && (self.ly as i16) < sy + sprite_height {
                    sprites += 1;
                    if sprites == 10 { break; }
                }
            }
        }
        let window_visible = self.lcdc & 0x20 != 0 && self.ly >= self.wy && self.wx <= 166;
        let sprite_penalty = sprites.saturating_mul(6);
        let window_penalty = if window_visible { 6 } else { 0 };
        (172u16 + sprite_penalty + window_penalty).min(289)
    }

    fn palette_read(&self, palette: &[u8; 0x40], index: u8) -> u8 {
        palette[(index & 0x3F) as usize]
    }

    fn palette_write(&mut self, bg: bool, value: u8) {
        if bg {
            let index = self.bgpi & 0x3F;
            self.bg_palette[index as usize] = value;
            if self.bgpi & 0x80 != 0 { self.bgpi = 0x80 | ((index.wrapping_add(1)) & 0x3F); }
        } else {
            let index = self.obpi & 0x3F;
            self.obj_palette[index as usize] = value;
            if self.obpi & 0x80 != 0 { self.obpi = 0x80 | ((index.wrapping_add(1)) & 0x3F); }
        }
    }

    fn cgb_palette_color(&self, bg: bool, palette: u8, color: u8) -> u32 {
        let base = ((palette & 0x07) as usize) * 8 + ((color & 3) as usize) * 2;
        let p = if bg { &self.bg_palette } else { &self.obj_palette };
        let raw = u16::from_le_bytes([p[base], p[base + 1]]) & 0x7FFF;
        let r5 = raw & 0x1F;
        let g5 = (raw >> 5) & 0x1F;
        let b5 = (raw >> 10) & 0x1F;
        let r = ((r5 << 3) | (r5 >> 2)) as u32;
        let g = ((g5 << 3) | (g5 >> 2)) as u32;
        let b = ((b5 << 3) | (b5 >> 2)) as u32;
        0xFF00_0000 | (r << 16) | (g << 8) | b
    }
}

fn dmg_shade(palette: u8, color: u8) -> u32 {
    let shade = (palette >> (color * 2)) & 3;
    let v = match shade { 0 => 224u32, 1 => 135, 2 => 79, _ => 30 };
    0xFF00_0000 | (v << 16) | (v << 8) | v
}
