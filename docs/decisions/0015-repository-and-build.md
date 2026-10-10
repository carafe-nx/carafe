# 15. Repository layout and builds in containers

Date: 2026-10-08

## Context

Carafe has three parts: the desktop app, the runtime that goes into each NSP and third-party code. The runtime and
hacBrewPack need Linux toolchains; the developer machine runs Windows.

## Decision

- **Layout.** `app/` — the Tauri app; `runtime/` — `loader/`, `overlay/`, `patches/autorun/` and a `Makefile`;
  `tools/hacbrewpack/` — hacBrewPack's build; `external/` — third-party code; `assets/` — each image once, as a finished
  file; `docs/` — this documentation.
- **Submodules** in `external/` are pinned to commits: `autorun` @ `636913c` (with `horizon-dlls` @ `5d6eccb`),
  `nx-hbloader` @ `82b9512`, `hacbrewpack` @ `745b16e` (rlaphoenix's mirror, tag `v3.05`). An update is an explicit
  commit. Only the containers fetch them: Git for Windows rewrites line endings.
- **Containers.** `docker/Dockerfile` is `FROM ghcr.io/autorunhq/switch-dev` pinned by tag and digest, plus llvm-mingw;
  `make` stops if the tag differs from `external/autorun/horizon-wine/switch-dev.txt`. `compose.yaml` has two services,
  `runtime` and `hacbrewpack`.
- **The runtime build** is GNU Make (`runtime/Makefile`, targets `fetch`, `wine`, `loader`, `layout`, `clean`). A
  patched copy depends on the original and its patch, so changing one overlay file does not rebuild the whole runtime. Output
  goes straight to `app/crates/carafe-desktop/resources/runtime/`; the app packs it into the archive.
- **Work folders** — `build/` inside each part, kept for incremental builds and cleaned with `make clean`.
- **npm scripts** from `app/`: `dev` and `build` build the runtime first; `dev:app` and `build:app` use the one already
  built; `runtime` builds only the runtime; `check` runs formatting, clippy, Rust tests and the TypeScript check.
- **Line endings.** `.gitattributes`: LF everywhere, `*.patch -text` so patches stay byte for byte, images binary.
  `.gitignore` also guards against NSP, NCA and XCI files.

## Rejected

- PowerShell scripts around Docker: logic spread over PowerShell, bash, Python and Rust.
- `cargo xtask` for the runtime: the image has no Rust.
- Forks under `carafe-nx` or vendored copies: a pinned commit is immutable, and copies grow the history.
- One `out/` folder for everything.
