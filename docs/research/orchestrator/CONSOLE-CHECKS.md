# Orchestrator: measurements for the console

There is one console run, at the end. Every measurement it needs is listed here, built into `[A57]` or the profiler
and checked in the lab before that run. Each line says which decision its number switches. The findings that asked
for each are named in the last column; the details of how to measure are there.

## Timing on the A57 (`[A57]`, at the third report, with the profiler)

| # | Measurement | Where it is logged | Decision it switches | From |
|---|---|---|---|---|
| K1 | Wake and hand-off: a ping-pong through `WaitForAddress`/`SignalToAddress` on one core, to an idle core, and to a core busy at 59 (waker at 58 and at 59); between cores 1↔0, 1↔2, 1↔3 | `[A57] wake` | spin or sleep in the front-end worker; ring batch size; the drain threshold; 58 or 59 for the main thread | horizon-scheduler, d3d9-front-end, cpu-graphics-split, threads-in-a-race |
| K2 | One cache line written on one core and read on another (1↔2, 1↔3), a 16 KB block, and an exclusive increment on a line last written by the other core | `[A57] lines` | one owning core for shared objects (reference counts) in the proxy | horizon-scheduler, main-thread-memory-stalls |
| K3 | Uncached memory (`svcSetMemoryAttribute` uncached): full-line and 4-byte writes, dependent and streaming reads, against cached; cost per line of a data-cache clean | `[A57] uncached` | cached staging patch in DXVK; whether DXVK's per-draw constant copy is worth reworking | shared-memory-attributes |
| K4 | TLB ladder: a chase over heap pages against pages forced to 4 KB entries by an alias, 64 KB to 64 MB | `[A57] tlb` | whether large pages for guest data are worth the Wine patch | memory-pages |
| K5 | Cycles per instruction against code size, 8–512 KB of straight-line and branchy code | `[A57] code` | where code layout ranks among the translation items | a57-translation |
| K6 | Indirect branch with periodic target patterns against a compare chain; `PRFM` forms and the latency of a prefetch issued on core 3 for core 1 | `[A57] branch`, `[A57] prefetch` | how jump tables are compiled; whether any prefetch from another core pays | a57-translation, run-ahead-prefetch |
| K7 | DRAM latency and stream bandwidth at the loading screen and every tenth race report | `[A57] dram` | whether the GPU's traffic slows the CPU's misses | memory-bandwidth |

## Profiler and runtime counters (every report)

| # | Measurement | Where it is logged | Decision it switches | From |
|---|---|---|---|---|
| P1 | Every thread registered, native ones too (wrapped libnx thread creation), and each core's idle ticks (`svcGetInfo`, idle tick count) | `[THREADS]`, new `[CORES idle]` | how much work each core may take; the system's share of core 3, handheld and docked | threads-in-a-race, core-3 |
| P2 | Core 3: `dxvk-cs` runnable-but-not-running time, longest system burst, jitter of a 63 thread | `[CORES idle]` | which tenants core 3 gets; when `dxvk-cs` moves to cores 0/2 | core-3, gpu-submission |
| P3 | Main-thread waits by kind (wrapped `horizon_futex_wait`, `horizon_server_sync_call`), the server lock's wait time, server requests per thread, wake-ups per thread | `[WAITS]` | whether the server lock needs splitting; whether message polling is worth batching | autorun-boundaries, threads-in-a-race |
| P4 | Block entry counts in FEX's blocks while sampling, written next to the samples; a race-only dump per thread | `jit-blocks.txt` | exact cycles per execution of every load: targeted `PRFM` and the DRAM share | main-thread-memory-stalls |
| P5 | Stage tag: walk the x86 frame chain to the frame-loop call site (found by the call that encloses `BeginScene`/`Present`) | `[PROF] stages` | inclusive shares of simulation, updates and render; whether frame splitting is ever worth a measurement | frame-stages |
| P6 | Share of samples in returns that missed FEX's call-return stack, with `SyscallReturnStack` on and off | `[PROF] fex outside blocks` | whether that patch comes back | a57-translation |
| P7 | FEX's own counters in process memory: compile count and time, lookup misses, map lock waits, SMC invalidations, disk-cache hits; long frames with the compile time inside them | `[FEX]` | the compile service's priority and its gain on stutter | translate-ahead, closed-loop-profile |
| P8 | PMU, only where the user has opened it: set A (cycles, instructions, L1D and L2 refills, L1D TLB refills, mispredicts) and set B (L1I refills, L1I TLB refills, indirect branches, loads, stores, event 0x12C), alternating | `[PMU]` | the front-end share against the 28 % estimate; DRAM share; order of layout against prefetch | pmu |
| P9 | Sampler from 1 % of a core instead of 3 %, one run | `[PROF]` | none; closes the threads not sampled | threads-in-a-race |

## Direct3D 9 and D3DX counters (with the profiler)

| # | Measurement | Where it is logged | Decision it switches | From |
|---|---|---|---|---|
| X1 | Per frame: locks by kind, flags, pool and bytes; bytes of draws from memory and of constants; `GetData` calls and `S_FALSE`; `GetRenderTargetData` and render-target locks; `Get*` calls; `Create*`/`Release`; DXVK's `CsSyncCount`/`Ticks`, `GpuSyncCount`/`Ticks`, `QueueSubmitCount`, chunks; states replayed per state-block apply | `[D3D9 sync]` | the drains per frame the proxy would make: build level 1 as is, add renaming and deferred readback first, or not at all; cached staging per game | cpu-graphics-split, d3d9-front-end, gpu-submission, shared-memory-attributes |
| X2 | D3DX per frame: Pass, ApplyPass and ApplyState entries, states per pass and the share unchanged, expressions evaluated with no changed input | `[D3DX]` | how much of D3DX's work is skippable (3.4.3); the work item size of level 2 | d3dx-in-exe, frame-stages |

## A/B with the stage-1 build

| # | Measurement | Where it is logged | Decision it switches | From |
|---|---|---|---|---|
| AB1 | Stage 1's placement and priorities on and off, same track | `[FPS]`, `[THREADS]` | whether the main thread at 58 alone on its core is kept | REPORT 3.2 |
| AB2 | SSE matrix multiply on and off | `[FPS]`, `[PROF]` | keep the slot change | d3dx-in-exe |
| AB3 | `NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=0` and `1` | `[THREADS]` `dxvk-cs`, `[FPS]` | NVK's push-buffer memory | shared-memory-attributes |

## In the lab, no console

| # | Measurement | Decision it switches | From |
|---|---|---|---|
| L1 | Main-thread memory trace through Eden's dynarmic memory callbacks (fastmem and page table off, the instruction's address recorded per access) into a model of the A57's caches and TLBs; also which lines the simulation writes and the render reads | miss rate per site; size of a render snapshot | main-thread-memory-stalls, frame-stages, run-ahead-prefetch |
| L2 | Dependence limit of parallelism from a DynamoRIO trace of the main thread on a PC (needs the owner's agreement to the download) | whether any loop is worth a speculative mode | speculative-parallelism |
| L3 | Correctness of each stage on every lab game | the stage may go to the console | all |
