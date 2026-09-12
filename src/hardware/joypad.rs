use super::interrupt::{Interrupt, Interrupts};

#[derive(Clone, Copy, Debug)]
pub enum Button { Right=0, Left=1, Up=2, Down=3, A=4, B=5, Select=6, Start=7 }

pub struct Joypad { select: u8, state: u8 }
impl Default for Joypad { fn default() -> Self { Self::new() } }
impl Joypad {
    pub fn new() -> Self { Self { select: 0x30, state: 0xFF } }
    pub fn read(&self) -> u8 {
        let v = 0xC0 | self.select;
        let mut low = 0x0F;
        if self.select & 0x10 == 0 { low &= self.state & 0x0F; }
        if self.select & 0x20 == 0 { low &= (self.state >> 4) & 0x0F; }
        v | low
    }
    pub fn write(&mut self, value: u8) { self.select = value & 0x30; }
    pub fn press(&mut self, button: Button, irq: &mut Interrupts) {
        let was_up = self.state & (1 << button as u8) != 0;
        self.state &= !(1 << button as u8);
        if was_up { irq.request(Interrupt::Joypad); }
    }
    pub fn release(&mut self, button: Button) { self.state |= 1 << button as u8; }
}
