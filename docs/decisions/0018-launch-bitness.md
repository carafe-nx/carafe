# 18. Launch bitness can be chosen by hand

Date: 2026-10-10

Extends [0008](0008-fixed-base-games.md).

## Context

Every NSP carries the same runtime: Wine's WoW64 runs 32-bit and 64-bit code alike and picks the mode from the `.exe`
at run time. Bitness reaches the NSP only through the process address space, which 0008 derives from the `.exe`
header. Wine front-ends (Lutris, Bottles, CrossOver, Winlator) let the player set bitness by hand and never detect it;
Carafe detects it and needs a way out for games the header misleads.

## Decision

- The launch step has a collapsed "Advanced" block with "Bitness: Auto (from the `.exe`) / 32 bit / 64 bit".
- The choice sets the address space: Auto keeps the rule of 0008, 32 bit gives type 2 (32 bit, no alias), 64 bit gives
  type 3 (39 bit) — `GameSource::address_space`.
- Choosing another `.exe` returns the setting to Auto and says so under the file list.
- Contradictions only warn, the build stays allowed: 32 bit for a 64-bit `.exe` (`X86OnX64`), 64 bit for an `.exe`
  that runs only at its own address (`X64OnFixedAddress`).
- `carafe.json` keeps the header's bitness in `arch` and the hand choice in `archOverride` (absent or `null` means
  Auto), so the format version stays 1. A rebuild restores the choice; the library card adds "by hand".

## Rejected

- Blocking the build on a contradiction: the player asked for an expert setting and owns the result.
- Hiding the option that contradicts the `.exe`: the setting would change its own choices with every file.
- A label-only setting that leaves the NSP unchanged: it would mislead the player.
