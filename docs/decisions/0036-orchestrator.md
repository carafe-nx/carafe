# 36. The orchestrator: one part of the runtime that decides how a game uses the Switch

Date: 2026-10-27

Research: [research/orchestrator/REPORT.md](../research/orchestrator/REPORT.md).

## Context

A translated Direct3D 9 game keeps one core busy and leaves the rest of the console idle: in NFSU2's race the main
thread is 94–99 % busy, the other three cores 60–95 % free, the GPU 46–73 % busy. Half of the main thread is the
graphics command producer (the D3DX effect framework linked into the `.exe` and DXVK's Direct3D 9 front end). Every
Wine thread runs at priority 59 because Windows priorities never reach Horizon, and the runtime's thread placement is
a name list in Autorun.

## Decision

- **One component, the orchestrator,** decides at start and while the game runs where each thread runs and at which
  priority, which mechanisms are on for the game, and what they are allowed. It plans from facts Carafe finds when it
  builds the NSP and from the previous run's profile, observes every report, decides in seconds, and acts through
  Horizon and flags that FEX and DXVK read.
- **Written in C,** in `runtime/orchestrator/`, built into `wine-nx-runtime.elf` like the overlay. Its decision logic
  takes no Horizon calls: inputs and actions go through a thin layer, so the logic also builds for the PC and is
  checked there against the console's logs.
- **On top of Autorun.** It reaches every point it needs through `--wrap` of runtime symbols, one-file Autorun
  patches, and Carafe's own FEX and DXVK patches. Mechanisms that live in FEX, DXVK or a proxy `d3d9.dll` stay in those
  layers, in their languages.
- **Placement by measurement, not by assumption.** A game with one dominant thread gets that thread alone on its core
  at 58, or at 59 with no other 59 there; a game with several busy threads gets them spread with wide masks. A thread
  goes above 59 only when it is known to block in its waits rather than spin. Core 3 takes work that tolerates the
  system's delays.
- **Staged, each stage behind its own Experimental row, off by default,** in the order of the report: measurements and
  the core; the graphics front end on another core; D3DX's unchanged work skipped and moved; translation; memory.

## Rejected

- **Rust for the core.** It would make nothing faster; most of the orchestrator's code and all of its gain are in C
  and C++ layers anyway (FEX, DXVK, the proxy, the wraps), and the one advantage, checking decisions on the PC, is kept
  by keeping the decision logic free of Horizon calls. It would add a toolchain, a second language in the runtime and
  the x18 register to guard.
- **Leaving Autorun.** Every control point is reachable on top of it. Reconsidered only if the server lock costs more
  than 1 % of the main thread, a game runs out of threads with the pool, or a rebase costs more than a week.
- **Lowering the main thread's priority** for single-threaded games. The main thread is the limit; anything that
  preempts it lengthens the frame.
- **A memory run-ahead thread and generic speculation in FEX.** DRAM is 3–7 % of the main thread; no software
  speculation scheme has sped up irregular code at this core-to-core latency.
