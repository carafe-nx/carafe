# Contributing to Carafe

Thanks for helping. This page covers how to build Carafe, how the repository is laid out and what a change
needs before it is merged.

Questions go to [Q&A](https://github.com/carafe-nx/carafe/discussions/categories/q-a), bugs to
[Issues](https://github.com/carafe-nx/carafe/issues). Security problems are reported privately through the
**Security** tab of the repository, not in a public issue.

## What you need

Carafe is built on Windows 10 or 11, x64 or ARM64:

- [Git](https://git-scm.com);
- [Docker Desktop](https://www.docker.com/products/docker-desktop/) — the Switch runtime and hacBrewPack are
  built only in containers;
- [Node.js](https://nodejs.org) 24 or newer;
- [Rust](https://rustup.rs) 1.99 with the Visual Studio C++ build tools that rustup asks for;
- WebView2, which Windows 11 already has.

## First build

```
git clone https://github.com/carafe-nx/carafe
cd carafe/app
npm ci
npm run dev
```

Clone without `--recursive`. The third-party code in `external/` comes as git submodules, and Git for Windows
would check them out with Windows line endings, which breaks the build. The containers fetch the submodules
themselves.

The first `npm run dev` downloads the build images, builds the Windows runtime for the Switch and hacBrewPack,
then opens the app. The runtime takes around ten minutes the first time; after that only what changed is
rebuilt.

## Commands

Run them from `app/`:

- `npm run dev` — builds the runtime and hacBrewPack, then opens the app in development mode;
- `npm run dev:app` — opens the app with the runtime already built;
- `npm run build` — builds everything and the installer, `app/target/release/bundle/nsis/`;
- `npm run build:app` — the installer with the runtime already built;
- `npm run runtime` — only the runtime, into `app/crates/carafe-desktop/resources/runtime/`;
- `npm run hacbrewpack` — only hacBrewPack, into `app/crates/carafe-desktop/binaries/`;
- `npm run check` — formatting, clippy, Rust tests and the TypeScript check;
- `npm run bindings` — regenerates the TypeScript types the UI gets from Rust.

The same builds run without npm: `docker compose run --rm runtime` and `docker compose run --rm hacbrewpack`
from the repository root. `make clean` in `runtime/` or `tools/hacbrewpack/` removes their build folders.

## Layout

- `app/` — the desktop app: Rust crates in `app/crates/`, the React UI in `app/ui/`;
- `runtime/` — what runs on the Switch: the loader, the overlay and the patches to Autorun;
- `tools/hacbrewpack/` — the build of hacBrewPack, the program that packs NSPs;
- `external/` — third-party code as pinned submodules, see [`external/README.md`](external/README.md);
- `docker/`, `compose.yaml` — the build containers;
- `assets/` — static images.

[`CLAUDE.md`](CLAUDE.md) and [`app/CLAUDE.md`](app/CLAUDE.md) describe the layers and the code style in detail.
They are written for AI coding agents and are just as useful for people.

## Code

- `npm run check` passes: `cargo fmt`, `cargo clippy` with warnings as errors, all tests, the TypeScript check.
- Public Rust items are documented with rustdoc: the contract, `# Errors`, `# Panics`. Function bodies have no
  comments.
- Every UI string comes from the translations and goes into both `en.json` and `ru.json`.
- Types the UI receives from Rust are generated; after changing them, run `npm run bindings` and commit the
  generated files.

## Changes to Autorun

`external/autorun` is never edited. A change to Autorun's sources is a patch in `runtime/patches/autorun/`:

- one patch changes one file;
- the name is numbered in order, `0012-short-description.patch`;
- it starts with a header: `Carafe: <what the patch does>`, a short paragraph on why, and the line
  `LGPL-2.1-or-later, as the file it changes.`;
- paths are relative to Autorun's root with `a/` and `b/` prefixes, as `git diff` writes them.

The build applies the patches to copies of the files, so the submodule stays clean. Updating a submodule to a
new commit is described in [`external/README.md`](external/README.md).

## Commits and pull requests

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org): one line in English,
  `type(scope): description`, for example `fix(pack): keep the icon when rebuilding`.
- Pull requests go to `main`. Keep one change per pull request and say how you tested it.
- Changes that affect the console are tested on the console.
- Firmware, games and links to them never go into issues, pull requests or discussions.

## License

By contributing you agree that your code is released under the license of the part it goes into:
GPL-3.0-or-later for Carafe as a whole, LGPL-2.1-or-later for `runtime/overlay/` and `runtime/patches/`,
ISC for `runtime/loader/`.
