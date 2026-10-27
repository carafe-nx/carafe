# Orchestrator: research questions

Each question gets its answer in `findings/<topic>.md`; the Answer column links it, with the conclusion in a few
words. A question with no link is open. New questions go at the end of their section. The synthesis is
[REPORT.md](REPORT.md).

## The console

| # | Question | Answer |
|---|---|---|
| C1 | How the Horizon scheduler behaves: preemption, migration between cores, priorities, yield; read in mesosphère | [horizon-scheduler](findings/horizon-scheduler.md): strict priority, 10 ms rotation only at 59 (cores 0–2) and 63 (core 3); idle cores take runnable threads; mutexes lend priority, address waits do not |
| C2 | Core 3: what the system runs there, what share is left to the application, handheld and docked; what owning more of it takes | [core-3](findings/core-3.md): all system modules live there at 15–58; a 63 thread gets the rest (35–90 %, to measure); Carafe's NSP already has core 3; owning more rejected |
| C3 | The cost of handing data between cores through L2 and of waking a sleeping thread, from sources | [horizon-scheduler](findings/horizon-scheduler.md): line 34–100 cycles, wake 1.5–5 µs on an idle core, up to 10 ms behind a busy 59; console check K1, K2 |
| C4 | Memory bandwidth for the CPU while the GPU draws; how the two share it | [memory-bandwidth](findings/memory-bandwidth.md): 21.3 GB/s, GPU 3–8 GB/s, main thread under 1.3; latency, not bandwidth, is the limit |
| C5 | Page sizes the application can map, and what large pages need | [memory-pages](findings/memory-pages.md): the kernel picks by alignment; heap and FEX arenas large, runtime image and guest data 4 KB; fix in the loader and Wine's mapping |
| C6 | The A57's PMU on the Switch: can the runtime read cache misses, mispredictions and stalls | [pmu](findings/pmu.md): yes, with `enable_user_pmu_access=1` in `exosphere.ini`, off by default; no stall events, stalls derived |
| C7 | Cached and write-combined memory shared with the GPU: what DXVK and NVK map, and what the CPU pays to read and write it | [shared-memory-attributes](findings/shared-memory-attributes.md): every DXVK CPU mapping is uncached; writes cheap in whole lines, reads cost DRAM; cached staging for games that read back |

## Where the time goes

| # | Question | Answer |
|---|---|---|
| T1 | Of the main thread's 67 % on loads and stores, what share waits on DRAM and what on L1 or L2, from the block dumps and the `[A57]` latencies | [main-thread-memory-stalls](findings/main-thread-memory-stalls.md): front-end 28 %, DRAM 3–7 %, L2 5–15 %, L1 chains 45–55 % |
| T2 | Every thread in a race run: what it does, what it waits on, who wakes it | [threads-in-a-race](findings/threads-in-a-race.md): the main thread waits for nothing; everyone waits for it; cores 0, 2, 3 are 60–95 % free |
| T3 | The game's frame as stages: input, simulation, D3DX effects, Direct3D calls, present; which depend on which | [frame-stages](findings/frame-stages.md): D3DX 30–35 %, Direct3D 9 17.5 %, render 12–25 %, simulation 3–15 %; one-way command stream |
| T4 | The upper bound of parallelism in the main thread if its dependencies were known exactly | [speculative-parallelism](findings/speculative-parallelism.md): method given (trace, dependence graph, per-loop limits); the numbers need lab check L2 |

## Ways to win

| # | Question | Answer |
|---|---|---|
| W1 | Splitting CPU and graphics work as Dolphin's dual-core mode and RPCS3 do, applied to a Direct3D 9 game under DXVK | [cpu-graphics-split](findings/cpu-graphics-split.md): level 1 at the device 1.10–1.17×; level 2 with D3DX effects 1.45–1.65×, GPU-capped |
| W2 | Pipelining frames: what one frame of latency buys and what stays in order | [frame-pipelining](findings/frame-pipelining.md): little alone; the worker's ring bounded at two Presents; the game's own frame split stays a measurement |
| W3 | The Direct3D 9 front end on another core in batches, unchanged states dropped on the game's side | [d3d9-front-end](findings/d3d9-front-end.md): 1.09–1.15× with state blocks mirrored and unchanged states dropped before queuing |
| W4 | The D3DX linked into a game's `.exe`: finding it, running it natively, running it on other cores | [d3dx-in-exe](findings/d3dx-in-exe.md): found by generic markers; SSE matrix slot 1.03×; skip unchanged work 1.15–1.25×; on the worker up to 1.35× with W3 |
| W5 | A run-ahead or helper thread that brings the main thread's data into L2 | [run-ahead-prefetch](findings/run-ahead-prefetch.md): 1.03–1.20× by the literature; with T1's 3–7 % DRAM share at most about 1.07×; targeted `PRFM` instead |
| W6 | Speculative execution and loop splitting in FEX: thread-level speculation, DSWP, HELIX, Janus | [speculative-parallelism](findings/speculative-parallelism.md): generic 1.00–1.05×, not worth building; per-loop mode 1.04–1.14× after T4 |
| W7 | Translated code for the A57: instruction choice by measured cost, block layout, `PRFM`, profile-guided translation | [a57-translation](findings/a57-translation.md): block edges, returns, inline caches, layout, second tier: 8–15 % together |
| W8 | Memory layout: large pages, hot data, false sharing | [memory-pages](findings/memory-pages.md): large pages 1–6 %; false sharing not a cost today; rules for new code |
| W9 | CPU work moved onto the GPU | [cpu-work-on-gpu](findings/cpu-work-on-gpu.md): 1–3 % at most; keep the GPU's idle time as headroom |
| W10 | Translation ahead of the game on a free core | [translate-ahead](findings/translate-ahead.md): feasible; stutter and start-up, not the race's average |
| W11 | Reading ahead from the SD card | [sd-read-ahead](findings/sd-read-ahead.md): race reads off the main thread; prefetch hides 0.7–1.4 s per load |
| W12 | A closed loop while the game runs and a per-game profile kept between runs | [closed-loop-profile](findings/closed-loop-profile.md): signals already sampled; profile beside the disk cache; slow loop, speed-only options |

## The design

| # | Question | Answer |
|---|---|---|
| D1 | Where Autorun is in the way, and what would need its design changed | [autorun-boundaries](findings/autorun-boundaries.md): on top of Autorun; wraps and one-file patches reach every point; Windows priorities never reach Horizon |
| D2 | The orchestrator's components, the layer each lives in, how they agree | [REPORT §3](REPORT.md#3-the-design) |
| D3 | The gain of each mechanism on the NFSU2 profile, and the order of the stages | [REPORT §5–6](REPORT.md#5-the-gain-worked-out): 1.43–2.29× on the main thread; GPU-bound or 60 fps |
| D4 | What the lab and the profiler need for each stage | [CONSOLE-CHECKS](CONSOLE-CHECKS.md) |
