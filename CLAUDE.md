# Carafe

Carafe is a desktop app that turns a Windows game folder into a standalone NSP for a Nintendo Switch
with custom firmware. The runtime inside every NSP is built on [Autorun](https://github.com/autorunhq/autorun):
Wine for Horizon OS with Box64/FEX, DXVK and Mesa.

## Layout

- `app/` — the Tauri app: Rust crates and the React UI. Layers and code style are in `app/CLAUDE.md`.
- `runtime/` — everything that runs on the Switch and goes into each NSP:
  - `loader/` — the loader, the NSP's first code (ISC, based on nx-hbloader);
  - `overlay/` — Carafe's code compiled into Autorun's runtime (LGPL-2.1-or-later);
  - `patches/autorun/` — changes to Autorun's sources, one file per patch (LGPL-2.1-or-later);
  - `Makefile` — builds the runtime into `app/crates/carafe-desktop/resources/runtime/`.
- `tools/hacbrewpack/` — builds hacBrewPack, the program the app runs to pack NSPs.
- `external/` — third-party code as git submodules pinned to exact commits. It is never edited:
  changes to Autorun go into `runtime/patches/autorun/`.
- `docker/`, `compose.yaml` — the build containers.
- `assets/` — static images, each stored once.

## Building

- The runtime and hacBrewPack build only in containers: `docker compose run --rm runtime` and
  `docker compose run --rm hacbrewpack`. Git for Windows must not fetch the submodules: it rewrites
  line endings. The containers fetch them.
- From `app/`: `npm run dev` and `npm run build` build the runtime first; `dev:app` and `build:app`
  use the runtime already built; `npm run runtime` builds only the runtime; `npm run check` runs
  formatting, clippy, Rust tests and the TypeScript check.

## Rules

- Firmware, games and NSPs never go into the repository.
- Text files use LF line endings; patches are stored byte for byte (`.gitattributes`).
- Commit messages follow Conventional Commits: one line in English, `type(scope): description`.
