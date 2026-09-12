<h1 align="center">gameboy-rs</h1>

<p align="center">
  <strong>A Game Boy and Game Boy Color emulator written in Rust.</strong><br>
  <sub>Low-level emulation, a pixel-art launcher, and a native ROM picker.</sub>
</p>

<p align="center">
  <a href="https://github.com/yunekoto-dev/gameboy-rs/blob/main/LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-2ea44f.svg"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust 2021" src="https://img.shields.io/badge/Rust-2021-f74c00.svg?logo=rust&logoColor=white"></a>
  <img alt="Platforms: Windows, Linux, macOS" src="https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-3978b8.svg">
</p>

## `01` — What is this?

`gameboy-rs` is a desktop emulator for original Game Boy (DMG) and Game Boy
Color (CGB) cartridges. It focuses on a small, readable Rust codebase and a
usable desktop experience: launch a ROM from the command line, or open the
retro launcher and choose one through the operating system's native file
picker.

The hardware model is detected from the cartridge header. ROMs are not
included with this project.

## `02` — Current status

| System | Status |
|---|---|
| Game Boy DMG | ✅ Playable baseline |
| Game Boy Color | ✅ Playable baseline |

The CGB implementation is functional, with minor PPU timing differences still
being refined. Compatibility will vary by game, mapper, and hardware feature.

## `03` — Highlights

- DMG and CGB hardware detection from the ROM header
- MBC1, MBC2, MBC3, and MBC5 cartridge support
- CGB VRAM and WRAM banking, palettes, speed switching, and HDMA paths
- SDL2 video and audio output
- Fixed-size pixel-perfect display window
- Native file picker for `.gb` and `.gbc` ROMs
- Headless execution for automated checks and experiments
- Cross-platform CI for Windows, Linux, and macOS

## `04` — Build and run

### Requirements

- [Rust](https://www.rust-lang.org/tools/install) with the stable toolchain
- A native C compiler and [CMake](https://cmake.org/)
- Linux: SDL2 development dependencies from your distribution, plus desktop
  dialog dependencies for the native file picker
- macOS: Xcode Command Line Tools and CMake
- Windows: Visual Studio Build Tools with the C++ workload and CMake

SDL2 itself is compiled by Cargo through the `bundled` feature. You do not need
to install SDL2 separately, but the platform build tools above are still
required.

Clone and build the project:

```bash
git clone https://github.com/yunekoto-dev/gameboy-rs.git
cd gameboy-rs
cargo build --release
```

Run a ROM directly:

```bash
# Linux or macOS
./target/release/gameboy-rs /path/to/game.gb

# Windows PowerShell
.\target\release\gameboy-rs.exe "C:\Games\game.gb"
```

Run without a ROM to open the launcher:

```bash
# Linux or macOS
./target/release/gameboy-rs

# Windows PowerShell
.\target\release\gameboy-rs.exe
```

For a non-graphical run, use headless mode:

```bash
cargo run --release -- --headless --cycles 1000000 path/to/game.gb
cargo run --release -- --headless --frames 60 path/to/game.gb
```

Save files are written next to the ROM by default. Use `--save PATH` to choose
a different location.

## `05` — Controls

| Keyboard | Game Boy input |
|---|---|
| Arrow keys | D-pad |
| `Z` / `X` | A / B |
| Right Shift | Select |
| Enter | Start |
| Escape | Quit |

The launcher also accepts Enter, Space, or a mouse click to open the ROM
picker.

## `06` — Development

Run the checks used by CI:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

The repository is organized around the emulator layers:

```text
src/
├── cartridge/    ROM parsing and memory bank controllers
├── cpu/          LR35902 CPU execution
├── hardware/     PPU, timer, interrupts, and joypad
├── apu.rs        Audio processing
├── bus.rs        Memory and hardware bus
├── emulator.rs   Frame and cycle orchestration
└── main.rs       CLI and SDL2 frontend
```

## `07` — Project notes

This is an educational and experimental emulator project. It is not intended
to distribute copyrighted ROMs, BIOS files, or proprietary game assets. Use
only software you are legally allowed to use.

Contributions, bug reports, and compatibility notes are welcome. When
reporting a game issue, include the ROM header information, platform, and the
command used to launch it.

## License

Released under the [MIT License](LICENSE).

Built by [Yunekoto](https://github.com/yunekoto-dev).
