# Building the NSP

How Carafe produces the NSP and the limits it has to respect. Facts about hacBrewPack are read from its code at
`v3.05` (commit `745b16e`, `external/hacbrewpack`) unless stated otherwise.

## Why hacBrewPack

No maintained library on any language builds a complete encrypted NCA (surveyed 2026-10-04):

- LibHac (C#, MIT) has RomFS, PFS0 and encryption builders, but no NCA, IVFC or CNMT builder;
- linkle (Rust) builds RomFS and PFS0 without IVFC; nsz-rs (Rust) only encrypts and writes PFS0;
- hacPack has sources only in archives; Autorun's forwarder builds NCAs on the console without encrypting sections.

So Carafe runs hacBrewPack as a separate program ([rlaphoenix/hacBrewPack](https://github.com/rlaphoenix/hacBrewPack),
GPL-2.0). It is built for Windows x64 and ARM64 in a container (`tools/hacbrewpack/Makefile`) and ships next to the
app. The output is a Program NCA (ExeFS + RomFS), a Control NCA and a Meta NCA.

## hacBrewPack behaviour

- mbedTLS 2.6.1 is bundled; there are no other dependencies.
- It **edits its input files in place** (`npdm.c:18,118`, `nacp.c:16,88,119`), so Carafe hands it a copy.
- The Title ID must be within `0x0100…`–`0x0fff…` (`npdm.c:86-94`); above `0x01ff…` it only warns.
- `--titleid` changes only ACI0 in the NPDM (`npdm.c:100`), not the ACID range. Carafe's loader NPDM is built with
  a range of exactly one Title ID, so Carafe writes the Title ID into all three fields itself and does not pass
  `--titleid`.
- The NSP is named `<titleid>.nsp` (`main.c:382`).
- For every RomFS file it prints `Writing <file> to <romfs>` (`romfs.c:441`); the builder counts progress by these
  lines. A file that fails to open gives `Failed to open <path>!` on stderr (`romfs.c:437`).

### Paths up to 259 bytes on Windows

Paths live in `MAX_PATH` buffers (`filepath.h:61-62`). hacBrewPack's own 4095 (`utils.h:23-25`) applies only where
the system does not define `MAX_PATH`, and on Windows it is 260. Names from `readdir` are converted to UTF-8
(`filepath.c:34-41`) and appended with `strcat` without a length check (`filepath.c:141-153`).

Checked on Windows: full paths of 258 and 259 bytes build, 260 and 270 fail with "Failed to stat"; a 230-character
path with 50 Cyrillic letters (280 bytes of UTF-8) fails too. The limit counts UTF-8 bytes, not characters.

Carafe checks every RomFS path before the build and stops with `pathTooLong` instead of failing halfway
(`app/crates/carafe-core/src/path_limit.rs`). For a library in `D:\Carafe` the prefix up to the game folder is
`D:\Carafe\.carafe\build\<Title ID>\in\romfs\switch\wine\drive_c\Games\` — 76 characters.

On macOS and Linux `MAX_PATH` is not defined and the buffer is 4095; macOS also limits paths to `PATH_MAX` of 1024
bytes. Not verified.

### Command-line arguments and non-Latin paths

hacBrewPack uses `main(int argc, char **argv)` (`main.c:53`), so on Windows the arguments arrive in the ANSI code
page, while `os_strcpy` (`filepath.c:10-32`) reads them as UTF-8. A library folder with Cyrillic letters failed with
"Failed to convert … to UTF-16!". File names inside the RomFS were fine: they come from `readdir` in UTF-16.

Carafe builds hacBrewPack with a manifest that sets `activeCodePage = UTF-8` (`tools/hacbrewpack/hacbrewpack.manifest`),
so `argv` arrives in UTF-8; the sources are not changed. Checked on ARM64 and x64: a library and a game folder with
Cyrillic paths build, and the RomFS matches the input. On Windows older than 10 1903 the manifest has no effect;
not verified.

## What Carafe writes itself

The rules are in `app/crates/carafe-core/src/npdm.rs`, `nacp.rs` and `pfs0.rs`.

- **NPDM**: the ACI0 offset is a `u32` at `0x70`, the ACID offset at `0x78`. In ACID the magic `ACID` is at `+0x200`
  and the Title ID range is two `u64` at `+0x210` and `+0x218`; in ACI0 the Title ID is at `+0x10`. In the loader
  template ACID is at `0x80` and ACI0 at `0x340`. The address space type is chosen from the game's `.exe` header
  (`app/crates/carafe-core/src/pe.rs`).
- **NACP**: `nacptool` and hacBrewPack's `--titlename` (`nacp.c:46`) fill names only in entries 0–11 of 16. The Title
  ID goes into `PresenceGroupId` (`0x3038`), `SaveDataOwnerId` (`0x3078`) and `LocalCommunicationId[0]` (`0x30B0`);
  `AddOnContentBaseId` is the Title ID + `0x1000` (`0x3070`). User save data is `0x3E00000` with a `0x180000`
  journal; device save data is 256 MB with a 64 MB journal (`0x3090`). Also `VideoCapture = 2`, `LogoType = 2`,
  `SupportedLanguageFlag = 1`.
- **PFS0**: the `PFS0` header, the file count and the size of the name table; entries of `0x18` bytes (data offset,
  size, name offset); the name table is padded to a multiple of `0x20` (`pfs0.c:121`). At most 16 files
  (`pfs0.c:11`). Carafe reads this header to show the library: `carafe.json` is the fourth file of the NSP.

## The boot splash

The splash the system shows while the game starts comes from two files in the Logo NCA, `NintendoLogo.png` and
`StartupMovie.gif` (Carafe's are in `assets/switch/`).

- Files of 384×192 and 384×180 crash the system process that shows the splash (`overlayDisp`, `010000000000100c`)
  with a Data Abort at address 0, taking the whole console down.
- 256×128 (PNG, RGB) and 256×80 (GIF, 32 frames) work in every run. Which size or side exceeds the limit is not
  verified, so treat 256×128 and 256×80 as required. hacBrewPack does not check the sizes.
- Handheld, the system shows both files scaled to about 2/3 (measured from video, not exact).
- The system also squashes `NintendoLogo.png` vertically, to about 0.55–0.65 of its height: it looks like fitting
  256×128 into the 256×80 slot (0.625). Carafe's logo file is therefore stretched 1.6 times vertically in advance.
  The loading screen draws the same two files the way the system does — scaled to 2/3, the logo squashed to 0.625
  (`SPLASH_*` in `runtime/overlay/carafe_loading.c`) — so the picture does not jump between the two screens. The
  positions were measured from video and are a few pixels off. Whether `StartupMovie.gif` is squashed too is not
  verified.
