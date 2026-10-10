# Testing on the console

How to check a change on a real Switch: build a game with Carafe, install it, launch it and read its logs.
There is no emulator loop: everything that runs on the Switch is tested on the console.

## Before the console

- `npm run check` from `app/` passes: formatting, clippy, Rust tests and the TypeScript check.
- `npm run dev` builds the runtime and hacBrewPack in containers and opens the app. After a change in `runtime/`
  this rebuilds the runtime; `npm run dev:app` opens the app with the runtime already built.
- The window can be driven without the mouse: `npm run cdp -- <command>` takes screenshots, clicks and reads the
  console of the running app ([`app/CLAUDE.md`](../app/CLAUDE.md), "Driving the window").

## The console

- Install with [DBI](https://github.com/rashevskyv/dbi) in MTP mode; Carafe copies the NSP over USB.
- Remove a test game in System Settings → Data Management, together with its save data, when the next test
  needs a clean start.
- After a test, keep a photo or video of the screen and the game's logs.

## A game NSP

1. In Carafe, build the game in the wizard. The app packs the runtime from
   `app/crates/carafe-desktop/resources/runtime/` and the game folder into the NSP; the game goes into RomFS
   under `switch/wine/drive_c/Games/<folder>`, and the path to its `.exe` into `/carafe/argv`.
2. Start DBI in MTP mode, connect the Switch and press **Install on Switch** in the game card: **To the SD card**
   or **To console memory**.
3. Launch the game from the HOME Menu. On the first launch the runtime may restart once: it installs codecs first
   and only then starts the game.

## Logs

The overlay keeps logs on the memory card in `sdmc:/switch/carafe/<Title ID>/`:

- `carafe-overlay.log` — the overlay: save data commits and files open for writing;
- `logs/autorun_runtime.log` — the runtime: Wine and the loading screen (`[CARAFE]` lines).

**Fetch logs** in the game card copies this folder to `.carafe/logs/<Title ID>` in the library folder and opens
it. DBI must be in MTP mode.

### Several launches in a row

The overlay keeps the logs of the last five launches: `X.log` is the latest, `X.1.log` the one before it, up to
`X.5.log`. The first line of `carafe-overlay*.log` is the date and time of the launch. When comparing launches,
write down for each one: the time, what was on the screen and how long you waited.

## Boot splash and loading screen

What to expect:

1. After a launch from the HOME Menu, the carafe and "Carafe" sit at the top left, the drop and circles at the
   bottom right (`assets/switch/NintendoLogo.png`, `assets/switch/StartupMovie.gif`). The carafe is not squashed.
   When the loading screen takes over, the carafe and the drop stay in the same place and at the same size.
2. After 1–5 s comes a dark screen: the game's name, the stage ("Starting the Windows environment" → "Starting
   the game" → "The game's first picture"), the seconds and the megabytes read, and "Carafe" at the bottom.
3. The loading screen goes away when the game shows its first picture. While it is up, buttons do not reach the
   game.
4. If nothing changes for 30 s, a yellow line "No progress for 30 s…" appears with a hint to press HOME. If the
   game hangs after it has taken the screen, the whole picture freezes, seconds included.

In `logs/autorun_runtime.log`:

- `[CARAFE] /carafe/loading/NintendoLogo.png 256x128` and
  `[CARAFE] /carafe/loading/StartupMovie.gif 256x80, 32 frames, 1600 ms` — the splash files were read;
  `no … in RomFS` or `… could not be decoded` means they were not;
- `[CARAFE] loading screen drawn at N s, WxH`;
- `[CARAFE] loading screen hidden after N s: <reason>`;
- `[CARAFE] no progress for 30 s, reporting the threads` — followed by the threads' state, for a hang.

## Loader errors

The loader's own errors show up as `2347-01xx` (module HomebrewLoader, `runtime/loader/source/main.c`):

- 101 — sm;
- 102 — fs;
- 103 — heap;
- 104 — session for the process handle;
- 105 — thread for the process handle;
- 106 — receiving the process handle;
- 107 — no process handle received;
- 108 — unloading the `.nro`;
- 109 — the path is not under `sdmc:/switch/wine/` or is missing from RomFS;
- 110 — RomFS;
- 111–114 — reading the `.nro`: read error, magic, layout, too large;
- 115 — mapping the code;
- 116 — memory permissions;
- 117 — return from the loaded program;
- 118 — exit.

## What to send with a bug

The [bug report form](https://github.com/carafe-nx/carafe/issues/new/choose) asks for it, and for the console
it comes down to:

- what happened and on which launch, with a photo or video of the screen;
- the Carafe version and the game with its version;
- the logs from **Fetch logs**, zipped.
