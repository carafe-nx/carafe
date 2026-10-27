# Carafe documentation

For contributors and the curious. Players need only the [README](../README.md).

- [architecture.md](architecture.md) — how a game folder becomes an NSP and how it starts on the console.
- [decisions/](decisions/) — decisions in force: what was chosen, why, and what was rejected.
- [autorun.md](autorun.md) — what Carafe relies on in Autorun: paths, settings, controls, exit, start-up hangs.
- [settings-menu.md](settings-menu.md) — the settings menu on the console: what the package tells it, where changes
  are kept.
- [nsp.md](nsp.md) — building the NSP: hacBrewPack limits, boot splash files.
- [testing.md](testing.md) — testing on a console.
- [research/fixed-base/REPORT.md](research/fixed-base/REPORT.md) — games with a fixed load address.
- [research/loading-screen/REPORT.md](research/loading-screen/REPORT.md) — the loading screen without black pauses.
- [research/updates/REPORT.md](research/updates/REPORT.md) — updating Carafe from GitHub releases.
- [research/save-commit/REPORT.md](research/save-commit/REPORT.md) — committing save data while files are open.
- [research/build-task/REPORT.md](research/build-task/REPORT.md) — a build as a cancellable task.
- [research/audio-source-rate/REPORT.md](research/audio-source-rate/REPORT.md) — no sound in games below 48 kHz.
- [research/threads/REPORT.md](research/threads/REPORT.md) — the 96-thread limit and what the runtime spends of it.
- [research/game-speed/REPORT.md](research/game-speed/REPORT.md) — where CPU time goes in a translated game and how to win
  it back.
- [research/orchestrator/REPORT.md](research/orchestrator/REPORT.md) — the orchestrator, a part of the
  runtime that makes a game use every resource of the Switch: the design, its gain, the stages and the console checks.
- [research/translated-code-cache/REPORT.md](research/translated-code-cache/REPORT.md) — translated code kept between
  runs: where it lives, preparation at the first start, the decisions so far.
- [research/in-game-menu/REPORT.md](research/in-game-menu/REPORT.md) — a settings menu over a running game.
- [research/game-registry/REPORT.md](research/game-registry/REPORT.md) — a game's Windows registry: where it lives, laying
  entries over it, `.reg` files.
- [research/languages/REPORT.md](research/languages/REPORT.md) — the console's languages and fonts, picking a language
  in the app and the installer.

## Keeping it

- A new decision is a new file in `decisions/`, numbered in order: context, decision, rejected options. A decision that
  replaces an older one says so, and the older file gets a line pointing to the new one.
- A fact about third-party code names the file, the lines and the commit it was read at; anything not checked is marked
  "not verified".
- Plans and their status live in [ROADMAP.md](../ROADMAP.md), not here.
- Everything is in English.
