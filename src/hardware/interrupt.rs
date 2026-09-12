#[derive(Clone, Copy, Debug)]
pub enum Interrupt { VBlank = 0, Stat = 1, Timer = 2, Serial = 3, Joypad = 4 }

pub struct Interrupts { pub flags: u8 }
impl Default for Interrupts { fn default() -> Self { Self::new() } }
impl Interrupts {
    pub fn new() -> Self { Self { flags: 0 } }
    pub fn request(&mut self, i: Interrupt) { self.flags |= 1 << i as u8; }
    pub fn read(&self) -> u8 { self.flags | 0xE0 }
    pub fn write(&mut self, value: u8) { self.flags = value & 0x1F; }
}
