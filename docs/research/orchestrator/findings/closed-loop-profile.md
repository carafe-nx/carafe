# W12. A closed loop while the game runs and a per-game profile kept between runs (the translator's side)

## Answer

The translator can feed a loop cheaply, because most of what it needs is already sampled or counted: the runtime's
profiler pauses the busiest threads every 2 ms, reads their registers and FEX's state pointer, and already finds the
FEX block and guest instruction under each sample (Autorun patches 0050, 0055, 0058). From the same samples it can
record, at no extra cost to the game, the hot blocks with their host offsets, the target of each sampled indirect
branch (the guest target is in a register at the lookup), whether returns miss FEX's call-return stack, and the x87
control word the code runs under. FEX's own per-thread counters (compile time and count, lookup misses, map lock
waits, SMC invalidations, disk-cache hits and misses) exist but are off on Horizon because their slots come from a
unix call Autorun does not provide. Put in process memory, they cost a few stores per compile. A per-game profile of
a few hundred KB (blocks by module and offset with heat and first-use order, call sites with their observed targets,
x87 precision seen, SMC events, and the options in force) can be written next to the disk cache, whole and checked,
learning from patch 0004's corruption. The next run uses it three ways: W10's worker compiles the list ahead (start-up
and stutter), a second tier retranslates the hot set with W7's profile-driven changes (the 8–15 % of main-thread
time estimated in W7 assumes this profile for items c and e), and the options that only change speed are chosen per
game from evidence. Options that can change results (single precision, TSO, SMC) are never switched by the loop
alone. Inside a run, the loop has one strong action: once the hot set has been stable for some seconds, retranslate
it in the background and swap it in at a quiet point. The rest (worker priority, cache prefetch) are small knobs.

## Evidence

FEX at `395b132` in `runtime/build/fex-src`; Autorun at `636913ca`; Carafe's patches in `runtime/patches`.

### What is already observed

- **Sampler** (`horizon-wine/source/thread_profile.c` with Autorun patches 0050, 0053, 0055, 0058, 0063): every 2 ms,
  the four busiest threads; under FEX it reads x28 (the thread state), the block header the state names, walks the
  block's RIP entries to the guest address, splits time outside blocks into block entry, lookup, link and compile,
  divide, x87 math, fallback and dispatcher, and writes the 256 busiest blocks with their sampled host offsets and
  guest and host bytes (`logs/jit-blocks.txt`). That file is already a hot-block profile, though rewritten in place
  and including loading time (exp9: written at 22745 and at 68969 samples, `SPEED2_profiler.log:980, 1336`).
- **Per-report lines** in `SPEED2_profiler.log`: `[THREADS]` (busy share and core of every thread), `[FPS]`,
  `[CLOCKS]` (CPU, GPU, memory clock, GPU busy), `[D3D9 calls]`, `[PROF]` per thread, `[PROGRESS]` (SD reads and
  writes, system calls). The `[PROGRESS]` fields `block_tests`, `invalidations`, `callret_clean`, `callret_dirty` are
  Box64's counters (`horizon-wine/source/runtime.c:1231-1251`) and stay 0 under FEX.
- **FEX's own counters** (`FEXCore/include/FEXCore/Utils/SHMStats.h:59-81`): per thread `AccumulatedJITTime`,
  `AccumulatedSignalTime`, `AccumulatedSMCCount`, `AccumulatedFloatFallbackCount`, `AccumulatedCacheMissCount`,
  `AccumulatedCacheReadLockTime`, `AccumulatedCacheWriteLockTime`, `AccumulatedJITCount`,
  `AccumulatedDiskCacheHitCount`, `AccumulatedDiskCacheMissCount`, `AccumulatedDiskCacheLookupTime`. They are
  incremented in `CompileBlock` (`Core.cpp:889, 908, 932, 940, 944`), in the lookup (`LookupCache.h:192-194, 232`)
  and in SMC handling (`horizon.patch:1077`). They exist only when `FEX_PROFILESTATS` is set and FEX gets a
  shared-memory slot area from the unix side (`WOW64/Module.cpp:600-606, 653-654`,
  `Windows/Common/SHMStats.cpp:28-37`); Autorun's FEX unix table answers its first seven calls with
  `STATUS_NOT_SUPPORTED` (`horizon-wine/source/fex_unix.c:131-135`; which index is the slot allocator was not
  checked). The time base on Horizon is `CNTPCT_EL0` (`SHMStats.h:18-30`), 19.2 MHz.

### What can be added to the sampler for free

The sampler already has the paused thread's registers and the block's host code.

- **Indirect targets per site.** When the sampled PC lies in a lookup sequence (`eor x0, xN, xN, lsl #4` up to the
  `br`/`blr`/`ret`, `BranchOps.cpp:204-239`), the guest target is in the register the `eor` reads (x20 or x21 in the
  dumps). Logging (block, site offset, target) gives a target histogram per site. In the race set 8.5 % of samples sit
  in such sequences (W7), about 40 a second on the main thread, enough to tell monomorphic call sites from switches
  within a minute of play.
- **Missed returns.** A sample in a lookup sequence that ends in `ret` is a return that missed the call-return stack
  (W7-M4).
- **x87 control word.** `CPUState::FCW` (`FEXCore/include/FEXCore/Core/CoreState.h:191`) is at a fixed offset from
  x28. A histogram of its precision-control bits per module, sampled with the PC, shows whether the game's x87 code
  runs at 24-bit precision (Direct3D 9 sets it unless `D3DCREATE_FPU_PRESERVE`) or switches, as this game's D3DX does
  around its own calls (game-speed report).
- **Header and lookup shares per report**, already split by patch 0058, as the loop's measure of what translation
  costs right now.

### Where the profile can live

- FEX's disk cache lives under `%LOCALAPPDATA%\fex-emu\DiskCache\<hash>\`, routed by the overlay to
  `sdmc:/switch/carafe/<Title ID>/`, later Cache Storage (translated-code-cache report). The hash covers FEX's format,
  its code-affecting settings and the host CPU (`DiskCache.cpp:395-420`), so it changes with every codegen option.
  The profile describes the game, not the code FEX made, so it belongs beside the cache folder, not inside it, and
  survives an option or Carafe update.
- Keys: a module by name, PE time stamp and image size (`ImageTracker.cpp:38`), a block by its offset from the
  module's base, so DLLs loaded at another base still match.
- Writing it safely, from patch `0004` (`runtime/patches/fex/0004`): the overlay holds writes back from the card, so a
  file's size lags behind what was written, and appends landed on earlier records; FEX's index was corrupted at
  18 MB of a 180 MB cache. The fix kept the file's own end, a hash over each record, and checks of every offset.
  A profile is small, so the simpler form is enough: write the whole file to a temporary name with a header (version,
  game identity, payload hash) and rename it, at exit and every few minutes, so a crash loses only the last minutes;
  on reading, a file whose hash fails is ignored.
- Size: 15000 blocks × (offset, heat, first-use rank) is under 200 KB; call-site histograms for a few thousand sites
  add about as much.

### What the next run can do with it

- **Compile ahead** (W10): the list, hottest and earliest first, to the worker as each module maps.
- **Second tier** (W7 c, e): superblocks through hot calls, inline caches seeded with the observed target (W7 b1),
  jump tables of hot switches as internal branches (W7 b3), hot blocks packed together in order of use. Compiled in
  the background, stored in the disk cache marked as second tier so the next run loads them directly.
- **Options per game**, only those that cannot change results: the L1 lookup table size, `MaxInst` and multiblock
  size, the worker's queue policy, whether to load the whole cache at start. Options that can change results stay
  under rules decided ahead and checked in the lab: single precision follows the Direct3D version and
  `FPU_PRESERVE` (game-speed report), and the FCW histogram can only turn it off (a game that runs much of its x87
  code at 53- or 64-bit precision), never on.
- **SMC history**: a module or page range that saw invalidations in earlier runs is left out of compile-ahead and of
  the second tier.

### Swapping in second-tier code while the game runs

The pieces exist in FEX: `GuestToHostMap::Erase` severs every direct link to a block (`LookupCache.h:119-129`,
delinkers `JIT.cpp:500-529`); `AddBlockMapping` inserts the new code (`LookupCache.h:100-109`); each thread's L1 and
call-return stack are cleared by `InvalidateThreadCachedCodeRange` (`Core.cpp:1090-1103`), which needs
`CodeInvalidationMutex` unique, the same path SMC invalidation takes. Old code is never freed on Horizon
(`SharedCodeBufferManager.cpp`, `~CodeBuffer` is empty there), so a thread still inside it, or returning into it
through a saved return address, stays correct. The cost is one unique lock and a few hundred relinks, once per swap.

### What the others do

HP Dynamo selects traces with counters at trace heads and keeps no profile between runs
([PLDI 2000](https://dl.acm.org/doi/10.1145/349299.349303)); a profile-guided scheme for indirect branches in a binary
translator ([Connection Science 2022](https://www.tandfonline.com/doi/full/10.1080/09540091.2022.2041555)) uses the
targets seen at each site to choose the code for it; Box64's `BOX64_PROFILE` picks predefined option sets per program
by hand, not from measurement ([USAGE.md](https://github.com/ptitSeb/box64/blob/main/docs/USAGE.md)).

## What this means for the orchestrator

What the orchestrator reads from the translator, every report (about 1 s), all from memory it already shares:

| Signal | Source | Cost |
|---|---|---|
| main-thread busy share, frame time, GPU busy | `[THREADS]`, `[FPS]`, `[CLOCKS]` | exists |
| block-entry, lookup and missed-return shares | sampler (0058, plus the `ret` split) | exists or a few lines |
| compile time and count per thread, lookup misses, map lock waits, SMC count, disk-cache hits and misses | FEX stats in process memory | a few stores per compile and per lookup miss |
| hot set and its stability (overlap of the top 64 blocks between reports) | sampler's block table | exists |
| per-site indirect targets, FCW histogram | sampler | a few lines |

What it decides with them:

1. While compile time is high (start, new areas): run W10's worker at a higher priority on core 3 or a free core.
2. When the hot set has been stable for about 10 s and the worker is idle: build the second tier for the top blocks
   and swap it in during a frame that ran short.
3. If a second-tier block is invalidated (SMC) or faults: drop it, mark it in the profile, keep the first tier.
4. At exit: write the profile; choose the next run's speed-only options from it.

The loop must stay slow (seconds) and act on translation only; it never changes code under the game thread except
through FEX's own invalidation path.

## Needs the console or the lab

| # | Measurement | How | Decision it switches |
|---|---|---|---|
| W12-M1 | FEX stats readable on the console | FEX `ProfileStats` with slots in process memory; `[PROF]` lines with the counters per thread | Whether the loop has compile and lookup signals; without it, W10's and W12's triggers fall back to the sampler only |
| W12-M2 | Profile written and read back | lab: write at exit and every 2 minutes, kill the game in the middle of a write, restart; the profile is either whole or ignored | Ships only after it |
| W12-M3 | Per-site targets from samples | lab: the sampler's site histogram against an exact count from an instrumented FEX build on the same route | Whether sampled targets are good enough to seed W7's inline caches |
| W12-M4 | FCW histogram | console and lab: precision-control bits per module, NFSU2 and one Direct3D 9 game created with `FPU_PRESERVE` | Whether the profile may turn single precision off per module |
| W12-M5 | Hot-set stability | console: overlap of the top 64 blocks between consecutive reports in the race | The loop's trigger for building the second tier (seconds of stability) |
| W12-M6 | Swap cost | lab, then console: time the unique lock is held and the main thread's stall during a swap of 64 and of 256 blocks | Whether swaps wait for a menu or a loading screen, or may run during play |

Not verified: that the guest target register at the lookup is always the one `eor` reads (true in every dump read,
x20 and x21); the cost of the FEX stats on the lookup-miss path; the stability of the hot set in a race.
