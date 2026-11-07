# Carafe

Carafe is a desktop app that turns a Windows game folder into a standalone NSP for a Nintendo Switch
with custom firmware. The runtime inside every NSP is built on [Autorun](https://github.com/autorunhq/autorun):
Wine for Horizon OS with Carafe's builds of FEX and DXVK, and Mesa.

## Layout

- `app/` — the Tauri app: Rust crates and the React UI. Layers and code style are in the app section below.
- `runtime/` — everything that runs on the Switch and goes into each NSP:
  - `loader/` — the loader, the NSP's first code (ISC, based on nx-hbloader);
  - `overlay/` — Carafe's code compiled into Autorun's runtime (LGPL-2.1-or-later);
  - `orchestrator/` — the orchestrator, compiled into the runtime with the overlay: it lays the game's threads
    out over the four cores (decisions 36, 42); its policy test runs before every runtime build;
  - `patches/autorun/` — changes to Autorun's sources, one file per patch (LGPL-2.1-or-later);
  - `patches/fex/`, `fex/` — changes to FEX and the build of its WoW64 module from them (MIT, decision 29);
  - `patches/dxvk/`, `dxvk/` — changes to DXVK and the build of its modules from them (Zlib, decision 30);
  - `mono/` — stages Wine Mono, which goes only into NSPs of games that use .NET (decision 54);
  - `winrt/` — registers the WinRT classes of Wine's DLLs in the runtime's registry;
  - `media/` — adds demuxers and decoders to the FFmpeg build of Autorun's media backend (decision 58);
  - `dshow/` — registers DirectShow's filter categories and filters in the runtime's registry (decision 58);
  - `patches/dxwrapper/`, `dxwrapper/` — changes to dxwrapper and its build from `external/dxwrapper` with Microsoft's
    compiler in its own container; it draws DirectDraw and Direct3D 1–7 through Direct3D 9 (Zlib, decision 59);
  - `Makefile` — builds the runtime into `app/crates/carafe-desktop/resources/runtime/`.
- `tools/hacbrewpack/` — builds hacBrewPack, the program the app runs to pack NSPs, with Carafe's changes from
  `tools/hacbrewpack/patches/`.
- `external/` — third-party code as git submodules pinned to exact commits. It is never edited:
  changes to Autorun go into `runtime/patches/autorun/`.
- `docker/`, `compose.yaml` — the build containers.
- `assets/` — static images, each stored once.
- `docs/` — how Carafe works, decisions in force, research. Rules for keeping it are in `docs/README.md`.

## Building

- The runtime and hacBrewPack build only in containers: `docker compose run --rm dxwrapper`, then
  `docker compose run --rm runtime`, and `docker compose run --rm hacbrewpack`. Git for Windows must not fetch the submodules: it rewrites
  line endings. The containers fetch them.
- From `app/`: `npm run dev` and `npm run build` build the runtime first; `dev:app` and `build:app`
  use the runtime already built; `npm run runtime` builds only the runtime; `npm run check` runs
  formatting, clippy, Rust tests and the TypeScript check.

## Rules

- Firmware, games and NSPs never go into the repository.
- Text files use LF line endings; patches are stored byte for byte (`.gitattributes`).
- Commit messages follow Conventional Commits: one line in English, `type(scope): description`.
- A decision with a reason goes into `docs/decisions/` as a new numbered file, in English.
- ROADMAP.md: one line per item with a `bug`/`game`/`feature`/`research` tag. Planned items go in priority
  order, bold while in progress; done items get `[x]` and go to the top of Done.

## Carafe app (`app/`)

Tauri 2: the core is Rust, the window is a single-page app on React + TypeScript + Vite.

## Layout

Rust and the UI sit side by side rather than one inside the other, as in GitButler:

```
app/
├── Cargo.toml          Cargo workspace, members are crates/*
├── package.json        npm workspaces (ui) and the Tauri CLI; commands run from here
├── crates/
│   ├── carafe-core/    the core
│   ├── carafe-<area>/  an adapter that grew out of an example
│   └── carafe-desktop/ the Tauri crate: tauri.conf.json, capabilities/, src/
└── ui/                 the single-page app: its own package.json, vite.config.ts, src/
```

New Rust code is a crate in `crates/`; a new JS package is an npm workspace member next to `ui/`.

## Layers

Dependencies point inward: `ui` → `carafe-desktop` commands → adapters → `carafe-core`.

- **`carafe-core` is the core.** The domain (`TitleId`, `BuildRecord`, `AutorunSettings`…), the
  `carafe.json` format, ports — traits for everything that touches the outside world (`GameStore`,
  `Packer`, `Device`, `FolderInspector`, `KeysChecker`) — and the use cases on top of the ports. The core
  is pure: its only dependencies are `serde`, `thiserror` and `ts-rs`; no Tauri, files, processes, time or
  randomness — all of that comes through ports or arguments. Every rule of the core is covered by a unit test.
- **Adapters** implement the ports: the file system, hacBrewPack, MTP. OS-specific code lives in one module
  of the adapter behind `#[cfg(target_os = …)]`; the rest of the code sees only the trait. Each adapter is its
  own crate `carafe-<area>`. `unsafe` is forbidden everywhere except `carafe-mtp/src/wpd.rs` (COM calls):
  `carafe-mtp` has its own lints with `unsafe_code = "deny"`, and that module allows it for itself.
- **`carafe-desktop` is the composition root.** It creates the adapters, puts `Services` into
  `tauri::State` and registers the commands. A command is a thin layer: take the input, call the core,
  turn the error into a `CommandError`. The core makes the decisions.
- **`ui` is the window.** It talks to the core only through `ui/src/shared/api`: typed wrappers over
  `invoke` and event subscriptions.

## Rust

- Edition 2024, default rustfmt; `cargo fmt --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` are clean.
- Invalid states are unrepresentable: domain values are newtypes checked in the constructor
  (`TitleId::parse`), variants are `enum`s rather than strings and flags.
- Errors: `thiserror` in crates, propagated with `?`; `unwrap` and `expect` live in tests.
- Public items are documented with rustdoc `///` and `//!`: the contract, `# Errors`, `# Panics` — no
  history or rationale. Function bodies have no comments.

## Tauri

- Commands are `async`; long work (building, installing) runs in `tauri::async_runtime::spawn_blocking`,
  progress goes out as events named `<area>://<what>` (`build://progress`).
- Types the window sees: `Serialize`/`Deserialize` with `#[serde(rename_all = "camelCase")]`
  and `ts_rs::TS` with `#[ts(export)]`. `cargo test` generates the TypeScript files in
  `ui/src/shared/api/bindings/`; they change only by regenerating.
- Window permissions are minimal, in `carafe-desktop/capabilities/`; a plugin is added with the list
  of permissions it needs.
- `prod.keys` is read from the path in the settings, only to check it and to pack. The key contents stay
  in the process memory: they never reach logs, events or `carafe.json`.

## React and TypeScript

- `strict`, real types instead of `any`. The whole app is checked with `npm run check` from `app/`.
- `ui/src` layers, dependencies top down: `app/` (router, providers) → `pages/<screen>/`
  (library, onboarding, wizard, settings) → `features/<feature>/` (Autorun settings editors,
  Switch state…) → `shared/{api,ui,i18n,theme}`. A layer imports only from the layers below; a feature
  does not import another feature.
- A component is a function; one exported component per file, the file is named after it (PascalCase);
  a hook is `useWhat.ts`.
- Styles are CSS Modules (`*.module.css`). Colours, radii, shadows and the font come only from the
  variables in `shared/theme/tokens.css`; the theme is the `data-theme` attribute on `<html>`.
- Every UI string is an i18next key; a new key goes into every one of the 15 files in `shared/i18n/locales/`,
  with that language's plural forms.

## Driving the window

`npm run dev` opens the window with the WebView2 debugging port 9222
(`crates/carafe-desktop/tauri.dev.conf.json`, development only). `npm run cdp -- <command>`
(`scripts/cdp.mjs`, the Chromium debugging protocol over Node's built-in WebSocket, no dependencies):
`screenshot <file.png>`, `eval <js>`, `click <selector>`, `type <selector> <text>`, `mouse <x> <y>`,
`drag <x1> <y1> <x2> <y2>`, `console [seconds]`, `goto <path>`, `dialog <path>`,
`shot-at <width> <file.png>` — a screenshot at another window width (the size override lasts one
connection). A second instance of the app does not open the port: WebView2 shares the browser process
with the first one.

The system "Browse" dialogs are Windows windows and cannot be clicked. `dialog <path>` before pressing
"Browse" answers the next `plugin:dialog|open`: Tauri's IPC is locked (`invoke`, `ipc`, `postMessage` are
read-only), but the request goes out through `window.fetch` to `http://ipc.localhost/…`, and that is what
gets answered. The command fails if the answer was not set up — then do not press "Browse": the real
dialog would open. Paths passed to `eval` use forward slashes: backslashes get lost on the way through
the shell.
