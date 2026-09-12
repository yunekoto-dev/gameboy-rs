use super::interrupt::{Interrupt, Interrupts};

pub struct Timer { div_counter: u16, tima: u8, tma: u8, tac: u8 }
impl Default for Timer { fn default() -> Self { Self::new() } }
impl Timer {
    pub fn new() -> Self { Self { div_counter: 0, tima: 0, tma: 0, tac: 0 } }
    pub fn read(&self, offset: u16) -> u8 {
        match offset { 0 => (self.div_counter >> 8) as u8, 1 => self.tima, 2 => self.tma, 3 => self.tac | 0xF8, _ => 0xFF }
    }
    pub fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => self.div_counter = 0,
            1 => self.tima = value,
            2 => self.tma = value,
            3 => self.tac = value & 0x07,
            _ => {}
        }
    }
    pub fn reset_div(&mut self) { self.div_counter = 0; }

    pub fn tick(&mut self, irq: &mut Interrupts) {
        let old = self.timer_input();
        self.div_counter = self.div_counter.wrapping_add(1);
        let new = self.timer_input();
        if old && !new {
            if self.tima == 0xFF {
                self.tima = self.tma;
                irq.request(Interrupt::Timer);
            } else {
                self.tima = self.tima.wrapping_add(1);
            }
        }
    }
    fn timer_input(&self) -> bool {
        if self.tac & 0x04 == 0 { return false; }
        let bit = match self.tac & 0x03 { 0 => 9, 1 => 3, 2 => 5, _ => 7 };
        self.div_counter & (1 << bit) != 0
    }
}
