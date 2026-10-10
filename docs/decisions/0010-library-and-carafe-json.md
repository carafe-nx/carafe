# 10. The library is a folder of NSPs with `carafe.json` inside

Date: 2026-10-05 – 2026-10-07

## Context

The app has to show built games, rebuild them with the same settings and keep their save data across reinstalls.

## Decision

- **Library.** Tiles are read from the NSPs in the library folder each time it opens: the PFS0 header, `carafe.json` and
  `icon.jpg` (`carafe-core/src/pfs0.rs`, adapter `carafe-library`). There is no separate catalogue, so NSPs can be
  copied, renamed or deleted by hand. NSPs without `carafe.json`, damaged ones and ones written by a newer Carafe are
  skipped. Of several NSPs of one game the newest build is shown.
- **`carafe.json`** is a fourth, unencrypted file in the NSP, next to the NCAs. It holds the full build settings, the
  runtime version and its own format version. hacBrewPack runs with `--keepncadir`, and Carafe writes the PFS0 itself:
  first to `<Title ID>.nsp.part`, then renamed. DBI installs such NSPs.
- **Settings snapshot.** App settings hold defaults for new games; a game stores a full snapshot of its values, so
  changing defaults never changes built games silently.
- **Rebuild** is the same wizard, filled in, with the same Title ID, so installing over keeps save data.
- **Title ID** is random, `0x05XXXXXXXXXX[02468ace]000`: an application's fourth hex digit from the end is even, add-ons
  are numbered from base + `0x1000`.
- **Old runtime.** When Carafe carries a newer runtime than the one in `carafe.json`, the card says so. The runtime
  version is the app version.
- **Work folder.** Temporary build files live in `.carafe/build` inside the library: same disk, the finished NSP moves
  by rename. Before a build Carafe needs four times the game plus runtime, plus 256 MB, free. A lock file guards a
  running build; leftovers are removed when the app starts.
- **Default library folder** is `Documents/Carafe`.

## Rejected

- A catalogue file: it drifts from the folder when NSPs are touched by hand.
- Reading the build back from `control.nca`: it holds only the NACP and icon, not the build settings.
- A settings file next to the NSP: easy to lose.
- Storing only differences from the defaults: a rebuild would quietly pick up new defaults.
