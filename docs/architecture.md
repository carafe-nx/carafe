# How Carafe works

Carafe turns a Windows game folder into an NSP that the Switch installs and starts like any other title. This page
follows a game from the PC to the first frame on the console. Why each piece is the way it is — in
[decisions](decisions/).

## The parts

- **The desktop app** (`app/`): Tauri with a Rust core and a React window. It reads the game folder, builds the NSP and
  copies it to the Switch over USB.
- **The runtime** (`runtime/`): everything that goes into each NSP apart from the game. Autorun's Wine for Horizon OS
  with Box64, FEX, DXVK and Mesa (`external/autorun`), plus Carafe's own pieces:
  - `runtime/loader/` — the loader, the NSP's first code;
  - `runtime/overlay/` — code compiled into Autorun's runtime: file overlay, save data commits, loading screen,
    hang diagnostics;
  - `runtime/patches/autorun/` — changes to Autorun's sources, one file per patch.
- **hacBrewPack** (`tools/hacbrewpack/`): the program the app runs to pack NCAs into an NSP.

The runtime and hacBrewPack are built in containers and ship inside the installer; players need neither Docker nor
build tools.

## From game folder to NSP

1. **Read the folder.** `carafe-folder` lists the `.exe` files for x86 and x86-64, reads their bitness, product and
   company names, and marks installers and `steam_api.dll`. The wizard suggests the largest `.exe` that is not an
   installer.
2. **Fill in the wizard.** Name, publisher, icon, controls, graphics and the rest of Autorun's settings. The window
   draws the icon itself, so the wizard shows exactly what reaches the Switch.
3. **Plan the build.** `carafe-core` turns the wizard's record into files:
   - `main.npdm` with the game's Title ID and address space: 39 bit, or 32 bit for games that need a fixed load
     address ([8](decisions/0008-fixed-base-games.md));
   - `control.nacp` with the name, publisher, icon and save data size;
   - `carafe/argv` — the runtime `.nro` and the path to the game's `.exe`;
   - `carafe/title` and `carafe/icon.jpg` for the loading screen;
   - `<exe>.keys.txt` and `<exe>.wine-nx.txt` next to the game: controls and runtime settings; `<exe>.args.txt` when
     the game has arguments.
4. **Check before writing.** Free space (four times the game plus runtime) and the length of every RomFS path
   (hacBrewPack on Windows stops at 259 bytes).
5. **Lay out the folders** in `.carafe/build/<Title ID>` inside the library: the runtime unpacked from its archive and
   checked file by file, the game copied to `switch/wine/drive_c/Games/<folder>` in RomFS, the loader as ExeFS `main`,
   the boot splash as the logo.
6. **Pack.** hacBrewPack builds the program and control NCAs. Carafe writes the NSP itself, the NCAs plus
   `carafe.json` with the build record, first to `<Title ID>.nsp.part`, then renamed into the library.
7. **Install.** With DBI open in MTP mode, the game card copies the NSP into DBI's install storage on the SD card or in
   console memory.

## What is inside the NSP

- **ExeFS**: Carafe's loader as `main` and `main.npdm`.
- **RomFS**:
  - `switch/wine/` — Autorun's runtime: `wine-nx-runtime.nro`, Wine's prefix with its DLLs, fonts and `nls`;
  - `switch/wine/drive_c/Games/<folder>/` — the game with its settings files;
  - `carafe/` — `argv`, `title`, `icon.jpg` and the splash files for the loading screen.
- **Control**: the NACP and an icon per supported language.
- **Logo**: `NintendoLogo.png` and `StartupMovie.gif`, the boot splash.
- **`carafe.json`**: unencrypted build settings for the library and for "Rebuild".

## From the HOME Menu to the first frame

1. **Boot splash.** The system shows Carafe's logo and animation from the NSP's logo section.
2. **Loader.** `runtime/loader/` gets the process handle, reads `carafe/argv`, and for 32-bit games makes sure the
   image range is free, restarting the application if the kernel took it. Then it loads `wine-nx-runtime.nro` from
   RomFS and jumps into it with the Homebrew ABI entries.
3. **Overlay.** Before the runtime's `main`, the overlay replaces the `sdmc:` device. Reads under
   `sdmc:/switch/wine/` come from save data or RomFS, writes go to device save data, logs and caches go to
   `sdmc:/switch/carafe/<Title ID>/` on the SD card ([4](decisions/0004-sdmc-overlay.md)).
4. **Loading screen.** The overlay draws Carafe's loading screen in place of Autorun's on-screen keyboard picture: the
   same logo as the splash, the game's icon and name, the start-up stages, seconds and megabytes read. Patches
   `0008`–`0011` keep the screen with the compositor until the game's first frame
   ([9](decisions/0009-loading-screen.md)).
5. **Wine.** The runtime starts the game's `.exe` straight away, without Autorun's launcher, with the settings from the
   game's `.keys.txt` and `.wine-nx.txt`.
6. **Play.** Save data is committed 2 s after the last write when nothing in it is open for writing, and again at exit.
7. **Exit.** When the game quits, the loader closes the application and the console returns to the HOME Menu.

## Logs

Each run writes to `sdmc:/switch/carafe/<Title ID>/`: the runtime's logs and the overlay's `carafe-overlay.log`, with
the last five runs kept; 32-bit games also get the loader's `carafe-loader.log`. **Fetch logs** in the game card
copies them to `.carafe/logs/<Title ID>/` in the library. How to test on a console — [testing.md](testing.md).

## Further reading

- [autorun.md](autorun.md) — what Carafe relies on in Autorun: paths, settings, controls, exit, hangs.
- [nsp.md](nsp.md) — building the NSP: hacBrewPack limits, boot splash files.
- [research/fixed-base/REPORT.md](research/fixed-base/REPORT.md) — games with a fixed load address.
- [research/loading-screen/REPORT.md](research/loading-screen/REPORT.md) — the loading screen without black pauses.
