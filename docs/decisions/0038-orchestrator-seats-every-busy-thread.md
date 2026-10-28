# 38. The orchestrator gives every busy thread a core of its own

Date: 2026-10-28

Replaced by [0042](0042-orchestrator-lays-out-all-four-cores.md).

Builds on [decision 36](0036-orchestrator.md) and [decision 37](0037-d3d9-front-end-inside-dxvk.md).

## Context

The first orchestrator knew one shape: a single dominant thread got a core and everything else moved off it; a second
busy thread made it hand placement back to the runtime's balancer. With the Direct3D 9 front end on its own thread a
race has two busy threads (the game's main thread at 97 %, `dxvk-frontend` at about 60 %), and with the effect library
on its own thread it has three. The balancer does not know that they form a pipeline and should not share a core.

## Decision

- **Seats.** Each period the orchestrator ranks the threads that may run on the game's cores by load. Every thread at
  30 % of a core or more takes a core of its own, the busiest first, as long as cores remain; a thread the program
  pinned keeps its core. Light threads move to the cores no busy thread holds, or, when every core is held, to the
  core of the lightest busy thread.
- **When it acts.** It starts placing when the busiest thread holds 85 % of a core for two periods, re-seats when the
  set of busy threads has changed for two periods, and hands back to the balancer after three periods below 60 % or
  when there are more busy threads than cores (shader compilation while loading).
- **Stable.** A thread that keeps its seat keeps its core; only threads that change seats move.
- The logic stays free of Horizon calls: it is checked by tests and by replaying the console's logs on the PC.

## Rejected

- **Spreading by wide masks only.** Horizon's rotation at priority 59 then moves busy threads between cores and puts
  two stages of the pipeline on one core for whole periods.
