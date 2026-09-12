use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::model::HardwareModel;

pub trait Mapper {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
    fn title(&self) -> &str;
    fn mapper_name(&self) -> &'static str;
}

#[derive(Debug)]
pub struct Cartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    mbc: Mbc,
    title: String,
    battery: bool,
    save_path: Option<PathBuf>,
    dirty: bool,
}

#[derive(Debug)]
enum Mbc {
    RomOnly,
    Mbc1 { rom_bank: u8, bank_high: u8, ram_enabled: bool, mode: u8 },
    Mbc2 { rom_bank: u8, ram_enabled: bool },
    Mbc3 { rom_bank: u8, select: u8, ram_enabled: bool, latch: u8, rtc: Rtc },
    Mbc5 { rom_bank: u16, ram_bank: u8, ram_enabled: bool },
}

#[derive(Debug, Clone)]
struct Rtc { base: u64, latched: [u8; 5] }
impl Rtc {
    fn new() -> Self { let now = unix_secs(); Self { base: now, latched: [0;5] } }
    fn latch(&mut self) {
        let now = unix_secs().saturating_sub(self.base);
        let days = now / 86_400;
        let hours = (now / 3_600) % 24;
        let mins = (now / 60) % 60;
        let secs = now % 60;
        self.latched = [secs as u8, mins as u8, hours as u8, (days & 0xFF) as u8, ((days >> 8) as u8) & 1];
    }
}
fn unix_secs() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Result<Self, CartridgeError> {
        if rom.len() < 0x150 { return Err(CartridgeError::TooSmall); }
        let title = String::from_utf8_lossy(&rom[0x134..0x144]).trim_matches('\0').trim().to_string();
        let cart_type = rom[0x147];
        let ram_size = match rom[0x149] { 0x00 => 0, 0x01 => 0x800, 0x02 => 0x2000, 0x03 => 0x8000, 0x04 => 0x20000, 0x05 => 0x10000, _ => return Err(CartridgeError::BadRamSize(rom[0x149])) };
        let battery = matches!(cart_type, 0x03 | 0x06 | 0x09 | 0x0F | 0x10 | 0x13 | 0x1B | 0x1E);
        let mbc = match cart_type {
            0x00 | 0x08 | 0x09 => Mbc::RomOnly,
            0x01..=0x03 => Mbc::Mbc1 { rom_bank: 1, bank_high: 0, ram_enabled: false, mode: 0 },
            0x05 | 0x06 => Mbc::Mbc2 { rom_bank: 1, ram_enabled: false },
            0x0F..=0x13 => Mbc::Mbc3 { rom_bank: 1, select: 0, ram_enabled: false, latch: 0, rtc: Rtc::new() },
            0x19..=0x1E => Mbc::Mbc5 { rom_bank: 1, ram_bank: 0, ram_enabled: false },
            other => return Err(CartridgeError::UnsupportedMapper(other)),
        };
        let ram = if matches!(&mbc, Mbc::Mbc2 { .. }) { vec![0; 0x200] } else { vec![0; ram_size] };
        Ok(Self { rom, ram, mbc, title, battery, save_path: None, dirty: false })
    }

    pub fn title(&self) -> &str { &self.title }
    pub fn cgb_compatible(&self) -> bool { self.rom.get(0x143).copied().unwrap_or(0) & 0x80 != 0 }
    pub fn model(&self) -> HardwareModel { if self.cgb_compatible() { HardwareModel::Cgb } else { HardwareModel::Dmg } }
    pub fn mapper_name(&self) -> &'static str { match &self.mbc { Mbc::RomOnly => "ROM ONLY", Mbc::Mbc1 {..} => "MBC1", Mbc::Mbc2 {..} => "MBC2", Mbc::Mbc3 {..} => "MBC3", Mbc::Mbc5 {..} => "MBC5" } }
    pub fn battery_backed(&self) -> bool { self.battery }
    pub fn set_save_path(&mut self, path: impl AsRef<Path>) { self.save_path = Some(path.as_ref().to_path_buf()); }
    pub fn load_save(&mut self, path: &Path) -> std::io::Result<bool> {
        self.save_path = Some(path.to_path_buf());
        if !self.battery { return Ok(false); }
        match fs::read(path) {
            Ok(data) => {
                let n = self.ram.len().min(data.len());
                self.ram[..n].copy_from_slice(&data[..n]);
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
    pub fn save(&mut self) -> std::io::Result<()> {
        if !self.battery { return Ok(()); }
        if let Some(path) = &self.save_path {
            if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
            fs::write(path, &self.ram)?;
            self.dirty = false;
        }
        Ok(())
    }

    fn rom_banks(&self) -> usize { (self.rom.len() / 0x4000).max(1) }
    fn rom_at_bank(&self, bank: usize, offset: usize) -> u8 { let b = bank % self.rom_banks(); self.rom.get(b * 0x4000 + offset).copied().unwrap_or(0xFF) }
    fn ram_index(&self, bank: usize, offset: usize) -> Option<usize> { let idx = bank * 0x2000 + offset; (idx < self.ram.len()).then_some(idx) }
}

impl Mapper for Cartridge {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                let bank = match &self.mbc {
                    Mbc::Mbc1 { bank_high, mode: 1, .. } => (*bank_high as usize) << 5,
                    _ => 0,
                };
                self.rom_at_bank(bank, addr as usize)
            }
            0x4000..=0x7FFF => {
                let bank = match &self.mbc {
                    Mbc::RomOnly => 1,
                    Mbc::Mbc1 { rom_bank, bank_high, mode, .. } => (((*rom_bank & 0x1F).max(1)) as usize) | if *mode == 0 { (*bank_high as usize) << 5 } else { 0 },
                    Mbc::Mbc2 { rom_bank, .. } => *rom_bank as usize,
                    Mbc::Mbc3 { rom_bank, .. } => ((*rom_bank & 0x7F).max(1)) as usize,
                    Mbc::Mbc5 { rom_bank, .. } => *rom_bank as usize,
                };
                self.rom_at_bank(bank, addr as usize - 0x4000)
            }
            0xA000..=0xBFFF => match &self.mbc {
                Mbc::RomOnly => self.ram.get(addr as usize - 0xA000).copied().unwrap_or(0xFF),
                Mbc::Mbc1 { ram_enabled, bank_high, mode, .. } if *ram_enabled => self.ram_index(if *mode == 1 { *bank_high as usize } else { 0 }, addr as usize - 0xA000).and_then(|i| self.ram.get(i)).copied().unwrap_or(0xFF),
                Mbc::Mbc2 { ram_enabled, .. } if *ram_enabled => 0xF0 | self.ram[(addr as usize - 0xA000) & 0x01FF],
                Mbc::Mbc3 { ram_enabled, select, .. } if *ram_enabled && (0x00..=0x03).contains(select) => self.ram_index(*select as usize, addr as usize - 0xA000).and_then(|i| self.ram.get(i)).copied().unwrap_or(0xFF),
                Mbc::Mbc3 { ram_enabled, select, rtc, .. } if *ram_enabled && (0x08..=0x0C).contains(select) => rtc.latched[(*select - 0x08) as usize],
                Mbc::Mbc5 { ram_enabled, ram_bank, .. } if *ram_enabled => self.ram_index(*ram_bank as usize, addr as usize - 0xA000).and_then(|i| self.ram.get(i)).copied().unwrap_or(0xFF),
                _ => 0xFF,
            },
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match &mut self.mbc {
            Mbc::RomOnly => {
                if let 0xA000..=0xBFFF = addr {
                    let i = addr as usize - 0xA000;
                    if i < self.ram.len() { self.ram[i] = value; self.dirty |= self.battery; }
                }
            }
            Mbc::Mbc1 { rom_bank, bank_high, ram_enabled, mode } => match addr {
                0x0000..=0x1FFF => *ram_enabled = value & 0x0F == 0x0A,
                0x2000..=0x3FFF => *rom_bank = (value & 0x1F).max(1),
                0x4000..=0x5FFF => *bank_high = value & 0x03,
                0x6000..=0x7FFF => *mode = value & 1,
                0xA000..=0xBFFF if *ram_enabled => {
                    let bank = if *mode == 1 { *bank_high as usize } else { 0 };
                    let i = bank * 0x2000 + (addr as usize - 0xA000);
                    if i < self.ram.len() { self.ram[i] = value; self.dirty |= self.battery; }
                }
                _ => {}
            },
            Mbc::Mbc2 { rom_bank, ram_enabled } => match addr {
                0x0000..=0x3FFF if addr & 0x0100 == 0 => *ram_enabled = value & 0x0F == 0x0A,
                0x0000..=0x3FFF if addr & 0x0100 != 0 => *rom_bank = (value & 0x0F).max(1),
                0xA000..=0xBFFF if *ram_enabled => { self.ram[(addr as usize - 0xA000) & 0x01FF] = value & 0x0F; self.dirty |= self.battery; }
                _ => {}
            },
            Mbc::Mbc3 { rom_bank, select, ram_enabled, latch, rtc } => match addr {
                0x0000..=0x1FFF => *ram_enabled = value & 0x0F == 0x0A,
                0x2000..=0x3FFF => *rom_bank = (value & 0x7F).max(1),
                0x4000..=0x5FFF => *select = value,
                0x6000..=0x7FFF => { if *latch == 0 && value == 1 { rtc.latch(); } *latch = value; }
                0xA000..=0xBFFF if *ram_enabled && *select <= 0x03 => {
                    let i = (*select as usize) * 0x2000 + (addr as usize - 0xA000);
                    if i < self.ram.len() { self.ram[i] = value; self.dirty |= self.battery; }
                }
                _ => {}
            },
            Mbc::Mbc5 { rom_bank, ram_bank, ram_enabled } => match addr {
                0x0000..=0x1FFF => *ram_enabled = value & 0x0F == 0x0A,
                0x2000..=0x2FFF => *rom_bank = (*rom_bank & 0x100) | value as u16,
                0x3000..=0x3FFF => *rom_bank = (*rom_bank & 0xFF) | (((value & 1) as u16) << 8),
                0x4000..=0x5FFF => *ram_bank = value & 0x0F,
                0xA000..=0xBFFF if *ram_enabled => {
                    let i = (*ram_bank as usize) * 0x2000 + (addr as usize - 0xA000);
                    if i < self.ram.len() { self.ram[i] = value; self.dirty |= self.battery; }
                }
                _ => {}
            }
        }
    }

    fn title(&self) -> &str { &self.title }
    fn mapper_name(&self) -> &'static str { self.mapper_name() }
}

impl Drop for Cartridge { fn drop(&mut self) { let _ = self.save(); } }

#[derive(Debug)]
pub enum CartridgeError { TooSmall, UnsupportedMapper(u8), BadRamSize(u8) }
impl fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self { Self::TooSmall => write!(f, "ROM is smaller than the 0x150-byte header"), Self::UnsupportedMapper(v) => write!(f, "unsupported cartridge type 0x{v:02X}"), Self::BadRamSize(v) => write!(f, "unsupported RAM size code 0x{v:02X}") }
    }
}
impl std::error::Error for CartridgeError {}
