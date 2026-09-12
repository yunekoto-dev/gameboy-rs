use std::f32::consts::PI;

pub struct Apu {
    regs: [u8; 0x30],
    frame_seq: u32,
    phase1: f32,
    phase2: f32,
    phase3: f32,
    phase4: u16,
    sample_clock: u32,
}

impl Apu {
    pub fn new(cgb: bool) -> Self {
        let mut regs = [0; 0x30];
        regs[0x14] = 0x77;
        regs[0x15] = 0xFF;
        regs[0x16] = if cgb { 0xF0 } else { 0xF1 };
        Self { regs, frame_seq: 0, phase1: 0.0, phase2: 0.0, phase3: 0.0, phase4: 0, sample_clock: 0 }
    }
    pub fn read(&self, addr: u16) -> u8 {
        let i = (addr - 0xFF10) as usize;
        if i < self.regs.len() { self.regs[i] } else { 0xFF }
    }
    pub fn write(&mut self, addr: u16, value: u8) { let i = (addr - 0xFF10) as usize; if i < self.regs.len() { self.regs[i] = value; } }
    pub fn tick(&mut self) { self.frame_seq = self.frame_seq.wrapping_add(1); self.sample_clock = self.sample_clock.wrapping_add(1); }
    pub fn generate_samples(&mut self, frames: usize) -> Vec<i16> {
        let mut out = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            let s = (self.sample() * 8192.0).clamp(-32767.0, 32767.0) as i16;
            out.push(s);
            out.push(s);
        }
        out
    }

    pub fn sample(&mut self) -> f32 {
        const SR: f32 = 44_100.0;
        let enabled = self.regs[0x16] & 0x80 != 0;
        if !enabled { return 0.0; }
        let mut out = 0.0;
        let f1 = square_frequency(self.regs[0x03], self.regs[0x04]);
        let f2 = square_frequency(self.regs[0x08], self.regs[0x09]);
        self.phase1 = (self.phase1 + f1 / SR) % 1.0;
        self.phase2 = (self.phase2 + f2 / SR) % 1.0;
        if self.regs[0x04] & 0x80 != 0 { out += if self.phase1 < 0.5 { 0.15 } else { -0.15 }; }
        if self.regs[0x09] & 0x80 != 0 { out += if self.phase2 < 0.5 { 0.15 } else { -0.15 }; }
        out += ((self.phase3 * PI * 2.0).sin()) * 0.03;
        self.phase3 = (self.phase3 + 220.0 / SR) % 1.0;
        self.phase4 = self.phase4.wrapping_add(1);
        out
    }
}

fn square_frequency(lo: u8, hi: u8) -> f32 {
    let raw = (((hi as u16) & 7) << 8) | lo as u16;
    if raw >= 2048 { return 1.0; }
    131_072.0 / (2048 - raw) as f32
}
