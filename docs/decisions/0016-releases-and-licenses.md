# 16. Releases and licenses

Date: 2026-10-08 – 2026-10-09

The installer format, the release assets and the runtime version are replaced by [0017](0017-updates.md).

## Context

Releases must be reproducible, immutable and easy to check. The code mixes Carafe's own work with code that runs inside
LGPL Wine and an ISC loader.

## Decision

- **Windows first.** Installers are `.msi` for x64 and ARM64, each in English and Russian. macOS and Linux are in the
  roadmap: their hacBrewPack is not built and MTP is not tested there.
- **Release flow.** Actions → Release → Run workflow on `main` with a tag. The runtime and hacBrewPack build on Linux,
  the installers on `windows-latest` and `windows-11-arm`; the result is a draft release with `SHA256SUMS` and build
  attestation. Publishing the draft creates the tag. A failed run leaves no tag behind.
- **Version** comes from the tag and is written into `app/Cargo.toml` only during the build; the code carries `0.0.0`.
  The runtime version is the app version, so games built with an older Carafe are marked as having an old runtime.
- **No CHANGELOG file**: each release's notes describe what changed for players.
- **Checks** on every push and PR: formatting, clippy, Rust tests, TypeScript and the committed bindings.
- **Licenses.** Carafe is GPL-3.0-or-later. `runtime/overlay/` and `runtime/patches/` are LGPL-2.1-or-later, like
  Wine, so the built runtime stays under Autorun's license. `runtime/loader/` is ISC, after nx-hbloader. hacBrewPack
  (GPL-2) is a separate program the app only runs. Licenses are named in `LICENSE`, `Cargo.toml` and `package.json`,
  not in file headers. The app has an About section with the copyright, the license and a link to the sources.
- **Contributions** come through pull requests; the owner pushes to `main` directly.
- **Discussions** hold questions, ideas and game compatibility; issues are for bugs.

## Rejected

- Building from a pushed tag: a failed build means deleting and re-pushing the tag.
- Building on release publication: an immutable release cannot take files afterwards.
- Publishing automatically: a mistake would be seen only after release.
- GPL-3.0-only: changing the version later would need every author's consent.
- MIT or Apache-2.0: allow closed forks.
