# Carafe app (`app/`)

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
- Every UI string is an i18next key; a new key goes into both `ru.json` and `en.json`
  (`shared/i18n/locales/`).

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
