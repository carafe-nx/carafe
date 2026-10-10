# 14. Autorun defaults for the Switch

Date: 2026-10-06

## Context

Without Autorun's launcher nothing picks settings for a game. The runtime reads them from `<exe>.keys.txt` and
`<exe>.wine-nx.txt` next to the game in RomFS. What each setting does is in [docs/autorun.md](../autorun.md).

## Decision

- Every value is written explicitly, not only the ones that differ from Autorun's defaults.
- **Thread sync — Horizon**: Autorun's default is Standard, but OpenTTD reached its menu in 4 of 5 runs with Horizon
  against 2 of 5 without. A comparison over 10 runs each is still to do.
- **Scaling — FSR 1.0, sharpness 40 %**: old games in low resolutions look sharper. Not yet checked on a console.
- **On-screen keyboard — on**, as in Autorun.
- **Everything else** as in Autorun: DXVK, official build, a fourth core for graphics, VSync, no frame limit, 5 % dead
  zones, controller mode.
- Nothing is hidden; options that do nothing in an NSP say so in their hint. Each setting has a plain-language hint.
- Saved defaults are never rewritten silently: Settings has "Restore recommended" with a confirmation.

## Rejected

- Hiding LSFG, DXVK builds, window output or limits above 60.
- Defaults exactly as in Autorun.
