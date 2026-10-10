# 13. The desktop app: Tauri, a pure core and adapters

Date: 2026-10-05 – 2026-10-07

## Context

The app reads a game folder, draws an icon, builds an NSP, talks to the Switch over USB and needs to look the same in
two languages and two themes.

## Decision

- **Tauri 2 with a React + TypeScript + Vite SPA** in the system WebView. Rust does the work, the WebView draws.
- **Clean architecture.** `carafe-core` holds the domain without I/O: PE parsing, NPDM, NACP, PFS0, `carafe.json`,
  path limits, Autorun settings files. Ports are traits; adapters (`carafe-folder`, `carafe-images`, `carafe-library`,
  `carafe-mtp`, `carafe-pack`, `carafe-steamgriddb`) implement them; `carafe-desktop` is the composition root. Rules
  are in `app/CLAUDE.md`.
- **Own PE reader** in the core: three fields are needed, no crate for that.
- **Icons are drawn by the window** on a canvas, so what the wizard shows is exactly what reaches the Switch. The core
  checks the result: baseline JPEG, 256×256, at most 128 KiB. Sources: the `.exe` icon scaled by whole numbers, the
  player's own image, or SteamGridDB through Rust (the API sends no CORS headers) with the player's own API key.
- **Build wizard**: folder → `.exe` → look → controls → graphics → build. A failed or closed build leaves nothing in the
  library. All Autorun settings are reachable; rare ones are folded under "Advanced".
- **Strings** live in translation files; English and Russian. The theme follows the system or is picked by hand.
- **Game windows are left alone**: a game that opens windowed shows Wine's frame; full screen is the game's own setting.

## Rejected

- Svelte, Leptos, Slint, Iced, egui: fewer libraries, or the look is harder to build.
- Image processing in Rust: a JPEG codec and font rendering to repeat what the WebView already does.
- `reqwest`: about twenty more crates.
- A Carafe-wide SteamGridDB key: open source would reveal it.
