use crate::apu::Apu;
use crate::cartridge::{Cartridge, Mapper};
use crate::hardware::{interrupt::{Interrupt, Interrupts}, joypad::Joypad, ppu::Ppu, timer::Timer};
use crate::model::HardwareModel;

#[derive(Clone, Copy, Debug, Default)]
struct Hdma {
    source: u16,
    dest: u16,
    remaining: u8,
    hblank: bool,
    active: bool,
}

pub struct Bus {
    pub cart: Cartridge,
    wram: [u8; 0x8000],
    hram: [u8; 0x7F],
    pub ppu: Ppu,
    pub timer: Timer,
    pub joypad: Joypad,
    pub interrupts: Interrupts,
    pub apu: Apu,
    io: [u8; 0x80],
    pub ie: u8,
    pub serial_output: Vec<u8>,
    dma: u8,
    model: HardwareModel,
    wram_bank: u8,
    double_speed: bool,
    key1_prepare: bool,
    speed_phase: bool,
    hdma: Hdma,
    /// Remaining normal-clock cycles in a CGB KEY1/STOP speed-switch pause.
    speed_switch_remaining: u16,
}

impl Bus {
    pub fn new(cart: Cartridge) -> Self {
        let model = cart.model();
        let mut bus = Self {
            cart,
            wram: [0; 0x8000],
            hram: [0; 0x7F],
            ppu: Ppu::new(model.is_cgb()),
            timer: Timer::new(),
            joypad: Joypad::new(),
            interrupts: Interrupts::new(),
            apu: Apu::new(model.is_cgb()),
            io: [0; 0x80],
            ie: 0,
            serial_output: Vec::new(),
            dma: 0,
            model,
            // FF70 reads back 0 at reset, and value 0 maps to physical WRAM bank 1.
            wram_bank: 0,
            double_speed: false,
            key1_prepare: false,
            speed_phase: false,
            hdma: Hdma { source: 0xFFF0, dest: 0x9FF0, remaining: 0, hblank: false, active: false },
            speed_switch_remaining: 0,
        };
        bus.io[0x50] = 0x01;
        // KEY0: bit 2 = DMG compatibility mode. Native CGB mode reads 0x00.
        bus.io[0x4C] = if model.is_cgb() { 0x00 } else { 0x04 };
        bus
    }

    pub fn model(&self) -> HardwareModel { self.model }
    pub fn is_cgb(&self) -> bool { self.model.is_cgb() }
    pub fn is_double_speed(&self) -> bool { self.double_speed }
    pub fn speed_switch_pending(&self) -> bool { self.speed_switch_remaining != 0 }

    pub fn read8(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cart.read(addr),
            0x8000..=0x9FFF => self.ppu.cpu_read_vram(addr - 0x8000),
            0xC000..=0xCFFF => self.wram[(addr - 0xC000) as usize],
            0xD000..=0xDFFF => {
                let bank = if self.is_cgb() {
                    (match self.wram_bank & 0x07 { 0 => 1, n => n }) as usize
                } else { 1 };
                self.wram[bank * 0x1000 + (addr - 0xD000) as usize]
            }
            0xE000..=0xEFFF => self.wram[(addr - 0xE000) as usize],
            0xF000..=0xFDFF => {
                let mapped = addr - 0x2000;
                self.read8(mapped)
            }
            0xFE00..=0xFE9F => self.ppu.cpu_read_oam(addr - 0xFE00),
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.ie,
        }
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => self.cart.write(addr, value),
            0x8000..=0x9FFF => self.ppu.cpu_write_vram(addr - 0x8000, value),
            0xC000..=0xCFFF => self.wram[(addr - 0xC000) as usize] = value,
            0xD000..=0xDFFF => {
                let bank = if self.is_cgb() {
                    (match self.wram_bank & 0x07 { 0 => 1, n => n }) as usize
                } else { 1 };
                self.wram[bank * 0x1000 + (addr - 0xD000) as usize] = value;
            }
            0xE000..=0xEFFF => self.wram[(addr - 0xE000) as usize] = value,
            0xF000..=0xFDFF => {
                let mapped = addr - 0x2000;
                self.write8(mapped, value);
            }
            0xFE00..=0xFE9F => self.ppu.cpu_write_oam(addr - 0xFE00, value),
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => self.write_io(addr, value),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.ie = value,
        }
    }

    /// `cpu_cycles` is expressed in the normal 4.194304 MHz clock domain.
    /// In CGB double-speed mode, the PPU still advances at the base clock,
    /// while CPU/timer/audio advance every CPU cycle.
    pub fn tick(&mut self, cpu_cycles: u8) {
        for _ in 0..cpu_cycles {
            // A CGB speed switch inserts a long CPU pause. During the pause the
            // display/HDMA keep running, but the system DIV/timer circuit is held.
            if self.speed_switch_remaining != 0 {
                self.ppu.tick(&mut self.interrupts);
                if self.is_cgb() && self.ppu.hblank_started {
                    self.hdma_hblank_step();
                }
                self.speed_switch_remaining -= 1;
                if self.speed_switch_remaining == 0 {
                    self.double_speed = !self.double_speed;
                    self.speed_phase = false;
                    self.timer.reset_div();
                }
                continue;
            }

            self.timer.tick(&mut self.interrupts);
            self.apu.tick();
            let advance_ppu = if self.double_speed {
                self.speed_phase = !self.speed_phase;
                !self.speed_phase
            } else {
                true
            };
            if advance_ppu {
                self.ppu.tick(&mut self.interrupts);
                if self.is_cgb() && self.ppu.hblank_started {
                    self.hdma_hblank_step();
                }
            }
        }
    }

    pub fn request_interrupt(&mut self, i: Interrupt) { self.interrupts.request(i); }

    pub fn switch_speed_if_armed(&mut self) -> bool {
        if !self.is_cgb() || !self.key1_prepare || self.speed_switch_remaining != 0 { return false; }
        // CGB speed switching is triggered by STOP, but the CPU then remains
        // halted for 2050 M-cycles = 8200 normal-speed T-cycles before the
        // clock actually changes.
        self.key1_prepare = false;
        self.speed_switch_remaining = 8200;
        true
    }

    fn read_io(&mut self, addr: u16) -> u8 {
        match addr {
            0xFF00 => self.joypad.read(),
            0xFF01 => self.io[0x01],
            0xFF02 => self.io[0x02] | 0x7E,
            0xFF04..=0xFF07 => self.timer.read(addr - 0xFF04),
            0xFF0F => self.interrupts.read(),
            0xFF10..=0xFF3F => self.apu.read(addr),
            0xFF46 => self.dma,
            0xFF40..=0xFF4B => self.ppu.read_reg(addr),
            0xFF4C => self.io[0x4C],
            0xFF4D => 0x7E | if self.double_speed { 0x80 } else { 0 } | if self.key1_prepare { 1 } else { 0 },
            0xFF4F => self.ppu.read_reg(addr),
            0xFF50 => self.io[0x50],
            0xFF51 => (self.hdma.source >> 8) as u8,
            0xFF52 => self.hdma.source as u8,
            0xFF53 => (self.hdma.dest >> 8) as u8,
            0xFF54 => self.hdma.dest as u8,
            0xFF55 => if self.hdma.active { (self.hdma.remaining.saturating_sub(1)) & 0x7F } else { 0xFF },
            0xFF56 => 0x3E,
            0xFF68..=0xFF6C => self.ppu.read_reg(addr),
            0xFF70 => 0xF8 | (self.wram_bank & 0x07),
            _ => self.io[(addr - 0xFF00) as usize],
        }
    }

    fn write_io(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF00 => self.joypad.write(value),
            0xFF01 => self.io[0x01] = value,
            0xFF02 => {
                self.io[0x02] = value;
                if value & 0x80 != 0 {
                    self.serial_output.push(self.io[0x01]);
                    self.interrupts.request(Interrupt::Serial);
                }
            }
            0xFF04..=0xFF07 => self.timer.write(addr - 0xFF04, value),
            0xFF0F => self.interrupts.write(value),
            0xFF10..=0xFF3F => self.apu.write(addr, value),
            0xFF40..=0xFF4B => {
                if addr == 0xFF46 { self.dma = value; self.do_dma(value); }
                else { self.ppu.write_reg(addr, value); }
            }
            0xFF4C => if self.is_cgb() { self.io[0x4C] = value & 0x80 },
            0xFF4D => if self.is_cgb() { self.key1_prepare = value & 1 != 0 },
            0xFF4F => if self.is_cgb() { self.ppu.write_reg(addr, value); },
            0xFF50 => self.io[0x50] = value,
            0xFF51 => if self.is_cgb() { self.hdma.source = ((value as u16) << 8) | (self.hdma.source & 0x00F0); },
            0xFF52 => if self.is_cgb() { self.hdma.source = (self.hdma.source & 0xFF00) | (value as u16 & 0xF0); },
            0xFF53 => if self.is_cgb() { self.hdma.dest = 0x8000 | (((value as u16) & 0x1F) << 8) | (self.hdma.dest & 0x00F0); },
            0xFF54 => if self.is_cgb() { self.hdma.dest = 0x8000 | (self.hdma.dest & 0x1F00) | (value as u16 & 0xF0); },
            0xFF55 => if self.is_cgb() { self.start_hdma(value); },
            0xFF56 => {},
            0xFF68..=0xFF6C => if self.is_cgb() { self.ppu.write_reg(addr, value); },
            0xFF70 => if self.is_cgb() { self.wram_bank = value & 0x07; },
            _ => self.io[(addr - 0xFF00) as usize] = value,
        }
    }

    fn do_dma(&mut self, page: u8) {
        let base = (page as u16) << 8;
        let mut tmp = [0u8; 0xA0];
        for i in 0..0xA0u16 { tmp[i as usize] = self.read8(base.wrapping_add(i)); }
        for (i, &v) in tmp.iter().enumerate() { self.ppu.write_oam(i as u16, v); }
    }

    fn start_hdma(&mut self, value: u8) {
        if self.hdma.active && self.hdma.hblank && value & 0x80 == 0 {
            self.hdma.active = false;
            return;
        }
        let blocks = (value & 0x7F).wrapping_add(1);
        self.hdma.remaining = blocks;
        self.hdma.hblank = value & 0x80 != 0;
        self.hdma.active = true;
        if !self.hdma.hblank {
            while self.hdma.active { self.hdma_transfer_block(); }
        }
    }

    fn hdma_hblank_step(&mut self) {
        if self.hdma.active && self.hdma.hblank { self.hdma_transfer_block(); }
    }

    fn hdma_transfer_block(&mut self) {
        if !self.hdma.active || self.hdma.remaining == 0 { self.hdma.active = false; return; }
        let mut tmp = [0u8; 0x10];
        for i in 0..0x10u16 { tmp[i as usize] = self.read8(self.hdma.source.wrapping_add(i)); }
        for (i, &v) in tmp.iter().enumerate() {
            let offset = self.hdma.dest.wrapping_sub(0x8000).wrapping_add(i as u16) & 0x1FFF;
            self.ppu.write_vram(offset, v);
        }
        self.hdma.source = self.hdma.source.wrapping_add(0x10) & 0xFFF0;
        self.hdma.dest = 0x8000 | (self.hdma.dest.wrapping_add(0x10) & 0x1FF0);
        self.hdma.remaining = self.hdma.remaining.saturating_sub(1);
        if self.hdma.remaining == 0 { self.hdma.active = false; }
    }
}
