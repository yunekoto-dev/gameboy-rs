pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod emulator;
pub mod hardware;
pub mod model;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;
pub const DMG_CLOCK_HZ: u64 = 4_194_304;
pub const CGB_CLOCK_HZ: u64 = 8_388_608;
