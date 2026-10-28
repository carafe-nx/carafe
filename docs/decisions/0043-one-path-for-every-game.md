# 43. One path for every game

Date: 2026-10-28

Replaces how [decision 29](0029-fex-built-with-carafe-patches.md) and
[decision 30](0030-dxvk-built-with-carafe-patches.md) pick the FEX and DXVK revisions.

## Context

Carafe offered a choice of Direct3D (DXVK, DXVK with VKD3D, Wine's own wined3d), of DXVK builds (the official one,
GPL Async, Sarek) and versions, of the processor emulator (FEX or Box64), of thread synchronisation (Horizon or Wine's
standard one) and of whether graphics workers may use core 3. Every speed-up of the last months lives in one of these
paths: the Direct3D 9 front end, the effect thread and the joined passes in Carafe's DXVK build; the x87, flag and
call-return work in Carafe's FEX build; the orchestrator on Horizon synchronisation with core 3. Every other
combination is slower and untested, and each one is a case to check and to fix.

## Decision

- **One path, not a choice.** Direct3D 8–11 always goes through Carafe's DXVK build, the processor through Carafe's
  FEX build, synchronisation through Horizon, and graphics workers may always use core 3. The settings that chose
  otherwise are gone from the app and from the console's settings menu.
- **The runtime picks by what the game uses.** wined3d stays inside the runtime for games that draw with DirectDraw,
  which DXVK does not translate; VKD3D stays for Direct3D 12. Neither is a setting.
- **FEX and DXVK are pinned by Carafe.** The runtime's build scripts name the revisions themselves (FEX
  `395b132`, FEX-2609; DXVK `b1a1c99`) instead of reading them from Autorun, so a newer Autorun never changes them.
  They move only in a new Carafe version, together with Carafe's patches moved onto them; every NSP keeps the runtime
  it was built with.
- **Box64 stays built but is never chosen.** Autorun's build refuses 64-bit games (`WINE_NX_AMD64`) without
  Box64's core, so turning it off would cost every 64-bit game or a change to Autorun's build. It is about 1–2 % of
  the runtime archive; the app no longer writes `cpu = box64` or Box64's file.
- **Old settings are read, not kept.** A value for a removed choice is accepted and mapped to the one path; the next
  save or build writes it no more, and the app shows once which settings of a game changed. NSPs built before keep
  running as they are.
- **D3DX is Carafe's.** A game that loads `d3dx9_xx.dll` always gets Wine's D3DX built with Carafe's patches, even if
  it ships Microsoft's DLL in its folder; the effect thread and joined passes extend to it. A game that links D3DX
  into its executable keeps the interception of the effect interface: its code cannot be swapped, but the interface is
  where the work is moved.
- **Tabs.** Controls, Graphics (window output moves here), System (DXVK and FEX, each as a group of pages) and
  Experimental.

## Rejected

- **Keeping the choices as an escape hatch.** A game that breaks on the one path is fixed in that path: the
  components are built by Carafe and patched for the Switch.
- **Removing wined3d and VKD3D.** DirectDraw games would stop working; VKD3D costs 6.5 MB and may run light Direct3D
  12 games.
- **Replacing a D3DX linked into a game's executable with Wine's.** The game calls into it directly and its structures
  differ from Wine's.
- **Smaller runtime as the reason.** Box64 is about 1–2 % of the runtime archive; nearly all of it is Wine's own DLLs.
- **Building without Box64.** Autorun ties 64-bit games to Box64's core in its build.
