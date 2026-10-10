# 5. The game's data lives in device save data

Date: 2026-10-04

## Context

Wine writes a registry, a user profile, configuration and the game's own saves. They can live on the SD card or in the
application's save data.

## Decision

Everything the runtime writes, apart from logs and caches (see [4](0004-sdmc-overlay.md)), goes to the application's
device save data. It is not tied to a user account, so the console does not ask to pick a user. The data is removed
together with the game and is visible in the console's data management.

The size is fixed in advance in the NACP: 256 MB plus a 64 MB journal (`DEVICE_SAVE_DATA_SIZE` and
`DEVICE_SAVE_DATA_JOURNAL_SIZE` in `app/crates/carafe-core/src/nacp.rs`).

The price: the runtime has to commit writes itself, and a crash loses everything since the last commit, game saves
included.

## Rejected

- A folder `sdmc:/switch/carafe/<Title ID>/` for everything: simpler and safer on crashes, but it stays on the card
  after the game is deleted, and the NSP stops being the only place the game lives.
- Account save data: the same commit rules, plus a user picker at every start.
