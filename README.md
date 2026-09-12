<p align="center">
  <h1 align="center">gameboy-rs</h1>
  <p align="center">A Rust Game Boy and Game Boy Color emulator with a retro picker UI.</p>
</p>

<p align="center">
  <a href="https://github.com/yunekoto-dev/gameboy-rs/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/yunekoto-dev/gameboy-rs/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2021-orange?logo=rust&logoColor=white">
  <img alt="Platforms" src="https://img.shields.io/badge/targets-DMG%20%7C%20CGB-2ea44f">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg">
</p>

## Status

| Machine | Status |
|---|---|
| Game Boy DMG | ✅ playable baseline |
| Game Boy Color | ✅ playable baseline; minor PPU timing quirks remain |

## Running

The emulator is designed to run on Windows, Linux, and macOS. Rust and a
native C toolchain are required. SDL2 is built automatically by Cargo through
the `bundled` feature, so SDL2 does not need to be installed separately.

On Linux, install a C compiler, CMake, and the SDL2 development dependencies
provided by your distribution. On macOS, install Xcode Command Line Tools and
CMake. The native file picker may also require desktop dialog dependencies
provided by your Linux distribution.

Launch a ROM directly from the command line:

```text
cargo build --release
```

Then run the executable produced for your platform:

```text
# Linux
./target/release/gameboy-rs /path/to/pokemon.gbc

# macOS
./target/release/gameboy-rs /path/to/pokemon.gbc

# Windows PowerShell
.\target\release\gameboy-rs.exe "C:\Games\pokemon.gbc"
```

Or run the executable with no arguments: a small
retro, Game-Boy-shaped window opens with a "PRESS ENTER TO SELECT A GAME"
prompt. Pressing Enter/Space, or clicking the window, opens your operating
system's native file picker filtered to `.gb`/`.gbc` files — pick a game and
it starts right away. Press Escape or close the window to quit without
picking anything.

The hardware model (DMG vs. CGB) is detected automatically from the ROM's
header, not from the file extension.

## Controls

| Key | Action |
|---|---|
| Arrow keys | D-pad |
| Z / X | A / B |
| Right Shift | Select |
| Enter | Start |
| Escape | Quit |
