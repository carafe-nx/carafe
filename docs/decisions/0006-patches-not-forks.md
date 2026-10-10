# 6. Changes to Autorun are patches, not a fork

Date: 2026-10-05

## Context

Some fixes can only be made inside Autorun's code: start-up hangs, games with a fixed load address, the loading screen.
Third-party code in `external/` is pinned and never edited.

## Decision

- Each change is a patch in `runtime/patches/autorun/`, one file per patch, LGPL-2.1-or-later like the file it changes.
  The header says what the patch changes and why.
- At build time the patch is applied to a copy of the original in the build directory, and the copy replaces the
  original in every Autorun target. The first patch to a file makes the copy; later patches to the same file are
  applied on top of it. A patch that does not apply stops the build.
- Patched copies see the headers of the original's directory (`-iquote`).
- One task has one implementation: when a new approach replaces an old one, the old code goes away.

## Rejected

- `git apply` inside `external/autorun` with a revert afterwards: it still edits third-party files.
- A copy of the whole Autorun tree for each build: 2.8 GB.
- Overriding functions only at link time (`--wrap`): it does not reach static functions.
- A fork of Autorun under `carafe-nx`: a pinned commit is already immutable, and a fork is one more thing to keep
  in step with upstream.
