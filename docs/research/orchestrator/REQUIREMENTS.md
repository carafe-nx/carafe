# Orchestrator: requirements

The orchestrator is a part of the runtime that, from the moment a game starts, decides how every resource of the
Switch is used, through every layer the game runs on: FEX (x86 to ARM64), Wine and Autorun, DXVK, Mesa and NVK,
Horizon. The game says what it wants done; the orchestrator decides where, when and how it is done, so that the
console gives everything it has and only the hardware itself is the limit.

This file is the brief for the research. It changes only with the owner's agreement. Questions and their answers are
in [QUESTIONS.md](QUESTIONS.md), measurements for the console in [CONSOLE-CHECKS.md](CONSOLE-CHECKS.md), findings in
`findings/`, one file each.

## The rule

How a game is written (one main thread, synchronous calls, x87, Direct3D 9) is the input, not the limit. The
orchestrator decides where work runs; it does not follow the game's structure.

"Impossible" is not an answer. Every obstacle gets a way around it, from research, console emulators or binary
translators. Where no ready way exists, the research says what one would need and gives the closest workable
approximation, with numbers.

## Resources, all of them

- CPU: four Cortex-A57 cores, the out-of-order pipeline, branch prediction; threads, priorities and
  synchronisation between cores.
- Core 3. Autorun's "four cores" mode (Carafe's "Fourth core for graphics", on by default) moves graphics workers
  (shader compilers, DXVK's command thread) to core 3 at priority 63, only when the Autorun forwarder is installed
  (`horizon-wine/source/thread_profile.c`, `offload_thread_locked`). In the 2026-10-27 race run `dxvk-cs` used
  35–40 % of it. To answer:
  - what the system runs on core 3 and how much of it is left to the application, handheld and docked;
  - which more of the orchestrator's work can go there (translation ahead of the game, a memory run-ahead thread,
    the Direct3D call front end, sound, the Wine server), at which priorities, without hurting the system or the
    game;
  - whether the game can own core 3, or most of it, for good, and what that takes (npdm rights, Atmosphère,
    anything else);
  - how the orchestrator works without the forwarder, on three cores.

  The orchestrator uses core 3 fully when it is there and works correctly without it.
- Caches: 48 KB L1I and 32 KB L1D per core, a shared 2 MB L2, the TLBs and page sizes; where code and data sit in
  them.
- Memory: LPDDR4 shared by the CPU and the GPU; latency, bandwidth, the CPU and the GPU competing for it; the
  attributes of memory both use (cached or write-combined).
- GPU: the Maxwell GM20B; its load, command submission, synchronisation with the CPU, shader compilation, CPU work
  moved onto it.
- Storage: the SD card and romfs, reading ahead.

## Directions to cover, not limited to them

- Turning one game thread into work for several cores: pipelines by frame and by stage; splitting CPU and graphics
  work as Dolphin's dual-core mode and RPCS3 do; speculative execution and loop splitting in the binary translator
  (thread-level speculation, DSWP, HELIX, Janus); a run-ahead or helper thread for memory; the game's library code
  (the D3DX linked into its `.exe`) moved to other cores.
- Translated code for the A57: instructions picked by their measured cost, block layout for the instruction cache,
  `PRFM` prefetches, profile-guided translation from earlier runs kept on the SD card.
- Memory: large pages against TLB misses, hot data layout, false sharing of cache lines, CPU and GPU bandwidth
  agreed between them.
- A closed loop: observe, decide, act while the game runs, and a per-game profile kept between runs.
- The A57's hardware counters (PMU): cache misses, mispredicted branches, stall cycles; whether the Switch can give
  them, through Atmosphère, a kernel patch or anything else.
- Where Autorun is in the way: what can be done on top of it and what would need its design changed.

## What the research hands over

1. Sources: the ARM Cortex-A57 Software Optimization Guide, Tegra X1 and Maxwell documentation, papers on
   parallelisation and binary translation, how emulators and translators do it (Dolphin, RPCS3, Yuzu, Rosetta 2,
   Box64, QEMU MTTCG). Every conclusion names its primary source.
2. A model of the console's resources with numbers: memory latencies and bandwidth, the cost of a hand-off between
   cores and of waking a thread, how the Horizon scheduler behaves. What is measured is used; what is not gets a
   measurement in [CONSOLE-CHECKS.md](CONSOLE-CHECKS.md).
3. The design: its components, the layer each lives in, how they agree with each other.
4. The gain of each mechanism, worked out on the NFSU2 profile, with the reasoning shown.
5. A staged plan, and what the lab and the profiler need to check each stage.

6. For the owner, in Russian and in plain words: what the orchestrator sees when a game starts, what it decides,
   how each decision goes through the layers, what it does with each resource, and how much of the console it
   wins.

## Order of work

During the research nothing goes into `runtime/orchestrator/`. What is built serves measuring and does not change how
a game runs: the measurements of [CONSOLE-CHECKS.md](CONSOLE-CHECKS.md) in `[A57]` and the profiler, one-off
prototypes that answer a question (such as a trace of the main thread's memory accesses in the lab), and the analysis
tools in `tools/analysis/`.

The mechanisms come after the research, in the order its estimates give. The first stage ships behind an
Experimental row, off by default, so the one console run checks the measurements and that stage together.

## Working without the console

There is no console during the research; it comes back for the final check. The sources are the logs already
taken (`D:\debug_switch\carafe\nfsu2-exp*`), the sources of every layer, including Atmosphère's mesosphère as the
reference for the Horizon kernel's behaviour, the ARM, NVIDIA and Switchbrew references, and the lab for
correctness.

A conclusion that depends on a number only the console can give is stated as a range with its condition: "with X
below N, A; otherwise B". The missing measurements make one final set: built into `[A57]` and the profiler, checked
in the lab, ready for one console run, each with the decision it switches.

## What is known

From [../game-speed/REPORT.md](../game-speed/REPORT.md) and the console runs of October 2026, NFSU2 at 1887 MHz:

- In the race 35–54 fps; the GPU is 46–73 % busy.
- The main thread is 96–99 % busy: the game's code 77 %, `d3d9.dll` 17 %, FEX's block entries 7 %. All threads
  together use 1.5 of four cores.
- On the A57: L1 4 cycles, L2 34, DRAM 377, a TLB miss 356, a mispredicted indirect branch 24.
- 67 % of the main thread's samples sit on loads and stores.
- Up to 17000 state calls a frame, 96–99 % of them unchanged.
- FEX emits 2.19 ARM64 instructions per x86 one in the game's hot blocks.

## Bounds

- Nothing in `external/` is edited; changes to FEX, DXVK and Autorun are patches in `runtime/patches/`.
- No code for one game.
- Clocks are never changed.
- The code lives in `runtime/orchestrator/`, built into the runtime next to `runtime/overlay/`.
- Every change of behaviour is checked in the lab and on the console.
