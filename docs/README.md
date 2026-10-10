# Carafe documentation

For contributors and the curious. Players need only the [README](../README.md).

- [architecture.md](architecture.md) — how a game folder becomes an NSP and how it starts on the console.
- [decisions/](decisions/) — decisions in force: what was chosen, why, and what was rejected.
- [autorun.md](autorun.md) — what Carafe relies on in Autorun: paths, settings, controls, exit, start-up hangs.
- [nsp.md](nsp.md) — building the NSP: hacBrewPack limits, boot splash files.
- [testing.md](testing.md) — testing on a console.
- [research/fixed-base/REPORT.md](research/fixed-base/REPORT.md) — games with a fixed load address.
- [research/loading-screen/REPORT.md](research/loading-screen/REPORT.md) — the loading screen without black pauses.
- [research/updates/REPORT.md](research/updates/REPORT.md) — updating Carafe from GitHub releases.

## Keeping it

- A new decision is a new file in `decisions/`, numbered in order: context, decision, rejected options. A decision that
  replaces an older one says so, and the older file gets a line pointing to the new one.
- A fact about third-party code names the file, the lines and the commit it was read at; anything not checked is marked
  "not verified".
- Plans and their status live in [ROADMAP.md](../ROADMAP.md), not here.
- Everything is in English.
