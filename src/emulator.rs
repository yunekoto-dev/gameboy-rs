use std::path::Path;
use std::time::Duration;

use crate::bus::Bus;
use crate::cartridge::{Cartridge, CartridgeError};
use crate::cpu::{Cpu, CpuError};
use crate::model::HardwareModel;
use crate::{DMG_CLOCK_HZ, SCREEN_HEIGHT, SCREEN_WIDTH};

pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
    save_loaded: bool,
}

#[derive(Debug)]
pub enum EmuError {
    Cartridge(CartridgeError),
    Cpu(CpuError),
    Save(std::io::Error),
}

impl std::fmt::Display for EmuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cartridge(e) => write!(f, "cartridge: {e}"),
            Self::Cpu(e) => write!(f, "cpu: {e}"),
            Self::Save(e) => write!(f, "save: {e}"),
        }
    }
}

impl std::error::Error for EmuError {}
impl From<CartridgeError> for EmuError { fn from(value: CartridgeError) -> Self { Self::Cartridge(value) } }
impl From<CpuError> for EmuError { fn from(value: CpuError) -> Self { Self::Cpu(value) } }

impl Emulator {
    pub fn from_rom(rom: Vec<u8>, save_path: Option<&Path>) -> Result<Self, EmuError> {
        let cart = Cartridge::new(rom)?;
        let mut bus = Bus::new(cart);
        let mut save_loaded = false;
        if let Some(path) = save_path {
            save_loaded = bus.cart.load_save(path).map_err(EmuError::Save)?;
        }
        let cpu = Cpu::post_boot(bus.is_cgb());
        Ok(Self { cpu, bus, save_loaded })
    }

    pub fn run_frame(&mut self) -> Result<(), EmuError> {
        self.bus.ppu.frame_ready = false;
        while !self.bus.ppu.frame_ready {
            self.step()?;
        }
        Ok(())
    }

    pub fn step(&mut self) -> Result<u64, EmuError> {
        // During the CGB KEY1/STOP speed-switch pause the CPU executes no
        // instructions; only the hardware clock domain continues to advance.
        if self.bus.speed_switch_pending() {
            self.bus.tick(1);
            return Ok(1);
        }
        let t = self.cpu.step(&mut self.bus)? as u64;
        self.bus.tick(t as u8);
        Ok(t)
    }

    pub fn run_cycles(&mut self, cycles: u64) -> Result<u64, EmuError> {
        let mut spent = 0;
        while spent < cycles {
            spent += self.step()?;
        }
        Ok(spent)
    }

    pub fn model(&self) -> HardwareModel { self.bus.model() }

    pub fn save_loaded(&self) -> bool { self.save_loaded }

    pub fn framebuffer(&self) -> &[u32] {
        &self.bus.ppu.frame
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        self.bus.cart.save()
    }
}

pub fn frame_duration() -> Duration {
    Duration::from_secs_f64(70224.0 / DMG_CLOCK_HZ as f64)
}

pub fn frame_u32(frame: &[u32]) -> Vec<u32> {
    debug_assert_eq!(frame.len(), SCREEN_WIDTH * SCREEN_HEIGHT);
    frame.to_vec()
}
