# 39. A game's linked effect library runs on a thread of its own

Date: 2026-10-28

Builds on [decision 37](0037-d3d9-front-end-inside-dxvk.md).

## Context

With the Direct3D 9 front end on its own thread, NFSU2's main thread in the race is still 97 % busy: about half of it
is the effect library (D3DX 9.0c) the game links into its executable, which re-sends every state of a pass, and the
calls into DXVK that this makes. Moving only DXVK left that half in place and gained about 3 %. Putting the effect
library on the same worker as DXVK would load that worker with about 90 % of today's main thread.

## Decision

- **A chain of two front ends inside DXVK.** The game thread queues its device calls and the effect methods it calls
  into the first; its worker, `d3dx-effects`, runs the effect library and replays the device calls through the
  device's public methods into the second front end of decision 37, whose worker, `dxvk-frontend`, runs DXVK. The
  shadow that drops unchanged states lives on `d3dx-effects`, the only thread that sees every device call in order.
- **The effect interface is found, not configured.** The table of 76–96 code pointers that the executable's code
  installs into objects, whose methods lie in the code that refers to the effect compiler's strings. Queueing needs
  the 86-method layout of D3DX 9.0c and each queued method must pop the argument bytes its signature has; otherwise
  the effects stay on the game thread.
- **What is queued:** the parameter setters with a copy of their data, `SetTechnique`, `Begin` (it answers the pass
  count it got for the same technique before), `BeginPass`, `CommitChanges`, `EndPass`, `End`. Every other public
  method, `Release` included, first waits for the queue, except the handle lookups of Wine's D3DX (`GetParameter` to
  `GetAnnotationByName`), which only read the effect's structure and are called at once.
- **An Experimental row of its own**, `d3d9.effectThread`, off by default, that works only with the front end row.
- The orchestrator of [decision 38](0038-orchestrator-seats-every-busy-thread.md) seats the three busy threads on
  cores 0–2.

## Rejected

- **The effect library on the DXVK worker.** One worker would carry both halves and become the limit.
- **Replacing the effect library.** It is code of the game; the queue moves it without changing what it does.
