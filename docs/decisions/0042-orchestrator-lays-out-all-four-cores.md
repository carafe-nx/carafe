# 42. The orchestrator lays threads out over all four cores

Date: 2026-10-28

Replaces the seats of [decision 38](0038-orchestrator-seats-every-busy-thread.md).

## Context

With the effect passes joined (decision 40), NFSU2's open world has the effect worker at 55 %, the game thread at
76 % and core 0 at 46 %, while core 3 is 99.5 % busy: `dxvk-cs` there takes 62 % and the system the rest. `dxvk-cs`
runs on core 3 at priority 63, below every system thread, so it gets what the system leaves; every stage before it
waits for it each frame. The orchestrator of decision 38 saw only cores 0–2 and threads at 30 % or more; it could not
move `dxvk-cs`, did not see the system's share of a core, and put every light thread on one core without adding their
loads up.

## Decision

- **Every core and its own load.** Each period the orchestrator reads how long each core was idle, core 3 included,
  and takes what no movable thread accounts for as that core's own load: the system, shader workers, threads the
  program pinned. Without the reading it assumes 35 % for the system on core 3.
- **Heaviest first, onto the core that ends up least busy.** The movable threads, a server connection thread counted
  with its client, go one by one from the heaviest to the core whose load with the thread is lowest. A thread stays on
  its core when that core is at most 3 points worse.
- **Core 3 runs at priority 63**, so it takes only its graphics workers and threads of 15–30 %: never the heaviest
  thread, no other thread above 30 % that the system could starve there, no small threads that would gain nothing.
- **When it acts.** It lays threads out when the busiest core is at 85 % or more and the new layout lowers it by 5
  points, for two periods running; it changes a layout under the same rule. After three periods with every core below
  60 % it hands back to the runtime's balancer and moves every thread it took onto or off core 3 back.
- The runtime moves a graphics worker off core 3 with its own priority back and a game thread onto core 3 at
  priority 63, the same way it offloads the graphics workers (Autorun patch 0072).
- The logic stays free of Horizon calls: tests, and a replay of the console's logs that recomputes each core's load
  after every move.

## Rejected

- **Seats for busy threads only.** The pipeline's limit moved to a thread on core 3 that the seats never saw.
- **Fixed capacity for core 3.** The system's share changes with what the console does; it is measured.
