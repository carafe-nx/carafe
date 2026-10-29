# 52. The speed rows are built in; two stay Experimental

Date: 2026-10-30

Follows [decision 51](0051-gpu-command-memory-cached-built-in-code-cache-row-dropped.md).

## Context

The Experimental section held every speed change of the orchestrator research behind a row, off until measured. The
console runs of 2026-10-29 and 2026-10-30 (Carbon, the same stretch, main thread's CPU time a frame and FEX's sampled
code) settled most of them: switches through tables, compact returns and return points without the header store each
take samples off FEX's lookups and block entries; the D3D9 workers and the orchestrator have been on in every check
since stage 3. Two did not show a gain: the indirect call cache costs more than the lookup it saves in Carbon (its check
4.1 % of the samples against 3.4 % for the plain lookup, with most calls through virtual tables going to changing
targets), and large pages left the frame time where it was (20.4 ms against 19.8 ms) while they leave holes in a 32-bit
game's address space.

## Decision

- **On for every game, without a row**, each still turned off by its key in the program's settings file:
  - FEX's jump tables (Autorun `0079`), compact returns and return points without the header store (Autorun `0080`);
  - DXVK's D3D9 front-end thread, effect thread, joined effect passes and managed buffer copies (DXVK `0015`);
  - the orchestrator (`orchestrator = 0` turns it off);
  - single-precision x87 for Direct3D 8 and 9 games, written by the app from the game's Direct3D version.
- **The profiler brings FEX's full RIP map with it** (Autorun `0081`): `FEX_GDBSYMBOLS` is 1 whenever the profiler is
  on, and stays off otherwise.
- **Experimental keeps two rows, off**: the indirect call cache and large pages. Their code stays in the runtime; a game
  where they help can turn them on.
