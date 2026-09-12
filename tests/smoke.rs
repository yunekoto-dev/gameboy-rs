use gameboy_rs::cartridge::{Cartridge, Mapper};
use gameboy_rs::cpu::Cpu;
use gameboy_rs::model::HardwareModel;
use gameboy_rs::bus::Bus;

#[test]
fn cpu_post_boot_state() {
    let cpu = Cpu::post_boot(false);
    assert_eq!(cpu.regs.pc, 0x0100);
    assert_eq!(cpu.regs.sp, 0xFFFE);
    assert_eq!(cpu.regs.af(), 0x01B0);
}

#[test]
fn rom_only_reads() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x134..0x13A].copy_from_slice(b"TEST01");
    rom[0] = 0x3E;
    rom[1] = 0x42;
    let cart = Cartridge::new(rom).unwrap();
    assert_eq!(cart.read(0), 0x3E);
    assert_eq!(cart.title(), "TEST01");
}

#[test]
fn cpu_runs_simple_program() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x134..0x138].copy_from_slice(b"TEST");
    rom[0x100..0x106].copy_from_slice(&[0x3E,0x42,0x06,0x24,0x80,0x76]);
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);
    let mut cpu = Cpu::post_boot(false);
    for _ in 0..10 { let c=cpu.step(&mut bus).unwrap(); bus.tick(c); if cpu.halted { break; } }
    assert_eq!(cpu.regs.a, 0x66);
}


#[test]
fn cgb_vram_and_wram_banks_are_independent() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0x80;
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);

    bus.write8(0x8000, 0x11);
    bus.write8(0xFF4F, 0x01);
    bus.write8(0x8000, 0x22);
    assert_eq!(bus.read8(0x8000), 0x22);
    bus.write8(0xFF4F, 0x00);
    assert_eq!(bus.read8(0x8000), 0x11);

    bus.write8(0xD000, 0x33);
    bus.write8(0xFF70, 0x02);
    bus.write8(0xD000, 0x44);
    assert_eq!(bus.read8(0xD000), 0x44);
    bus.write8(0xFF70, 0x01);
    assert_eq!(bus.read8(0xD000), 0x33);
}

#[test]
fn cgb_palette_registers_auto_increment() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0x80;
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);

    bus.write8(0xFF68, 0x80);
    bus.write8(0xFF69, 0x1F);
    bus.write8(0xFF69, 0x00);
    bus.write8(0xFF68, 0x00);
    assert_eq!(bus.read8(0xFF69), 0x1F);
    bus.write8(0xFF68, 0x01);
    assert_eq!(bus.read8(0xFF69), 0x00);
}


#[test]
fn cgb_cartridge_selects_color_model() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0x80;
    let cart = Cartridge::new(rom).unwrap();
    assert_eq!(cart.model(), HardwareModel::Cgb);
    assert!(cart.cgb_compatible());
}

#[test]
fn cgb_speed_switch_is_armed_by_key1() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0x80;
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);
    assert!(!bus.is_double_speed());
    bus.write8(0xFF4D, 1);
    assert_eq!(bus.read8(0xFF4D) & 1, 1);
    assert!(bus.switch_speed_if_armed());
    assert!(bus.speed_switch_pending());
    assert!(!bus.is_double_speed());
    for _ in 0..32 {
        bus.tick(255);
    }
    bus.tick(40);
    assert!(bus.is_double_speed());
    assert_eq!(bus.read8(0xFF4D) & 0x80, 0x80);
}

#[test]
fn oam_bytes_are_independent() {
    use gameboy_rs::hardware::ppu::Ppu;

    let mut ppu = Ppu::new(false);
    for i in 0..0xA0u16 {
        ppu.write_oam(i, i as u8);
    }
    for i in 0..0xA0u16 {
        assert_eq!(ppu.read_oam(i), i as u8);
    }
}


#[test]
fn cgb_bg_remains_visible_when_lcdc_priority_bit_is_clear() {
    use gameboy_rs::hardware::ppu::Ppu;
    let mut ppu = Ppu::new(true);
    // CGB treats LCDC.0 as the BG/OBJ master-priority flag, not a BG enable bit.
    ppu.write_reg(0xFF40, 0x80);
    assert_eq!(ppu.read_reg(0xFF40) & 1, 0);
}

#[test]
fn cgb_mode3_blocks_cpu_vram_access() {
    use gameboy_rs::hardware::ppu::Ppu;
    use gameboy_rs::hardware::interrupt::Interrupts;
    let mut ppu = Ppu::new(true);
    ppu.write_vram(0x0000, 0x12);
    ppu.write_reg(0xFF40, 0x80);
    let mut irq = Interrupts::new();
    for _ in 0..80 { ppu.tick(&mut irq); }
    assert_eq!(ppu.mode() as u8, 3);
    assert_eq!(ppu.cpu_read_vram(0), 0xFF);
    ppu.cpu_write_vram(0, 0x34);
    assert_eq!(ppu.read_vram(0), 0x12);
}

#[test]
fn ppu_mode3_length_is_initialized_and_used() {
    use gameboy_rs::hardware::ppu::Ppu;
    use gameboy_rs::hardware::interrupt::Interrupts;
    let mut ppu = Ppu::new(true);
    let mut irq = Interrupts::new();
    for _ in 0..80 { ppu.tick(&mut irq); }
    assert_eq!(ppu.mode() as u8, 3);
    for _ in 0..172 { ppu.tick(&mut irq); }
    assert_eq!(ppu.mode() as u8, 0);
}

#[test]
fn cgb_post_boot_and_key0_state() {
    let cpu = Cpu::post_boot(true);
    assert_eq!(cpu.regs.af(), 0x1180);
    assert_eq!(cpu.regs.bc(), 0x0000);
    assert_eq!(cpu.regs.de(), 0xFF56);
    assert_eq!(cpu.regs.hl(), 0x000D);

    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0x80;
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);
    assert_eq!(bus.read8(0xFF4C) & 0x04, 0);
    assert_eq!(bus.read8(0xFF70) & 0x07, 0);
    bus.write8(0xD000, 0x5A);
    bus.write8(0xFF70, 0);
    assert_eq!(bus.read8(0xD000), 0x5A);
    bus.write8(0xFF70, 3);
    bus.write8(0xD000, 0xA5);
    bus.write8(0xFF70, 0);
    assert_eq!(bus.read8(0xD000), 0x5A);
    bus.write8(0xFF70, 3);
    assert_eq!(bus.read8(0xD000), 0xA5);
}


#[test]
fn cgb_key0_reports_native_mode() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0xC0;
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);
    // Native CGB mode is selected with KEY0 bit 2 cleared.
    assert_eq!(bus.read8(0xFF4C) & 0x04, 0);
}


#[test]
fn cgb_speed_switch_has_hardware_pause_and_resets_div() {
    let mut rom = vec![0u8; 0x8000];
    rom[0x143] = 0x80;
    let cart = Cartridge::new(rom).unwrap();
    let mut bus = Bus::new(cart);
    bus.write8(0xFF04, 0xAA);
    bus.write8(0xFF4D, 1);
    assert!(bus.switch_speed_if_armed());
    assert!(bus.speed_switch_pending());
    assert!(!bus.is_double_speed());
    for _ in 0..32 {
        bus.tick(255);
    }
    bus.tick(39);
    assert!(bus.speed_switch_pending());
    assert!(!bus.is_double_speed());
    bus.tick(1);
    assert!(!bus.speed_switch_pending());
    assert!(bus.is_double_speed());
    assert_eq!(bus.read8(0xFF04), 0x00);
}
