# Changelog

## 0.7.0
- Removed the experimental GBA core and all `.gba` support; the emulator now targets Game Boy (DMG) and Game Boy Color (CGB) only.
- Added a retro, Game-Boy-shaped picker window shown when the executable is launched with no ROM argument, letting you browse your computer for a `.gb`/`.gbc` file via the OS's native file dialog.
- Removed the now-unused `--debug`/`--log`/`--trace` CLI flags (they only ever instrumented the GBA core).

## 0.6.12
## 0.6.11

- Correct GBA IWRAM mirroring across the full 0x03xxxxxx region.
- Rework CpuFastSet to use 8-word block semantics, rounded word counts, and overlap-safe block reads.
- Correct CpuSet to use the full 21-bit count field.

# Changelog

## 0.6.11

- Fixed GBA BIOS `CpuFastSet` length semantics: R2 is a total 32-bit word count, not a count of 32-byte blocks.
- `CpuFastSet` now always performs 32-bit transfers and snapshots the fixed source word once.
- Added a regression test covering the `0x01001F00` startup pattern used by Super Mario Advance 2.

# Changelog

## 0.6.9
- Fixed debug format strings that were split into invalid macro arguments.
- Fixed GBA startup borrowing of the game code string.
- Fixed GBA CPU error propagation into the public emulator error type.
- Fixed mutable/immutable borrow conflict when dumping the recent instruction trace.
- Added explicit CLI help entries for GBA debugging flags.

## 0.6.8
- Added GBA diagnostic tracing: CPU instructions, frame statistics, IO/events and recent-instruction dumps.
