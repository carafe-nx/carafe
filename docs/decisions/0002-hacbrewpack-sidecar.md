# 2. hacBrewPack packs the NSP

Date: 2026-10-04, updated 2026-10-08

## Context

No maintained library builds complete NCAs: LibHac never created them, and Rust has only fragments.
hacBrewPack builds a whole NSP from folders and streams large files.

## Decision

- The app runs hacBrewPack as a separate program, a Tauri sidecar from `app/crates/carafe-desktop/binaries/`, with no
  console window. It ships with its GPL-2 license and a link to its sources.
- It is built from the pinned submodule `external/hacbrewpack` in `tools/hacbrewpack/`, inside the official
  `mstorsjo/llvm-mingw` image pinned by digest, for Windows x64 and ARM64. Its sources are not changed.
- The Windows build carries a UTF-8 manifest (`activeCodePage`): hacBrewPack reads arguments in the ANSI code page and
  fails on paths with non-Latin letters otherwise. Works on Windows 10 1903 and later.
- hacBrewPack cannot read a file path longer than 259 bytes of UTF-8 on Windows. The app checks every RomFS path before
  it unpacks anything (`carafe-core/src/path_limit.rs`) and reports the longest one.
- hacBrewPack is not built in Autorun's `switch-dev` image: it runs on the PC, not on the Switch.

## Rejected

- Building NCAs in Rust or with Ryujinx's LibHac in C#.
- Running hacBrewPack inside the app process: it calls `exit()` on errors.
- MSVC: the sources expect MinGW (`unistd.h`, `dirent.h`).
- Patching `main.c` to `wmain`: third-party code stays unchanged.
