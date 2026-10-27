# W5. A run-ahead or helper thread that brings the main thread's data into L2

## Answer

A helper thread on another A57 core can only move a line into the shared 2 MB L2. It turns a DRAM miss (377
cycles) into an L2 hit (34), saving at most 91 % of each miss it covers. It cannot help loads that miss L1 and hit
L2 (34 cycles, which only a `PRFM` in the main thread's own code can hide), and it cannot fill the main core's private
TLBs. The literature on helpers that share only a cache level with the main core, measured on real hardware, gives
1.15× on high-miss programs and slowdowns on others (Lee, Jung, Lim, Solihin, Core Duo with a shared 2 MB L2), far
below the 1.5–1.8× of simulated SMT helpers. Software prefetching on this exact core (Tegra X1, A57 at 1.9 GHz) gave
1.1× on average for indirect-access kernels (Ainsworth & Jones). The gain is

    speedup = 1 / (1 − m × c × 0.91)

where m is the share of the main thread's time stalled on DRAM and c the share of those misses the helper covers in
time. With m = 0.10–0.30 (not measured; the 67 % of samples on loads and stores includes 20 % on the stack, almost all
L1 hits) and c = 0.3–0.6 (the measured helper coverage range), the gain for NFSU2 is **1.03–1.20×**, most likely
about 1.05–1.08×. The approach that fits a game best is not a backward slice but frame-correlated prefetch: frames
repeat, so a helper replays the previous frame's miss addresses, paced by a progress counter the runtime already has
(Direct3D calls per frame). It needs the miss addresses, which without the PMU's data-address sampling come from the
lab's trace (T4's S1) or from sampled FEX instrumentation. The first step is a measurement of m, not a helper.

## Evidence

Numbers marked "body" were read in the paper's text or tables; "abstract" means not verified; "figure only" means the
numbers exist only as charts.

### Helpers that share the core (SMT): the upper end

- Collins, Wang, Tullsen, Hughes, Lee, Lavery, Shen, "Speculative precomputation: long-range prefetching of
  delinquent loads", ISCA 2001, https://cseweb.ucsd.edu/~tullsen/isca2001.pdf (body). Simulated in-order SMT
  Itanium, memory 115 cycles. "commonly 10 or fewer static loads cause more than 80% of L1 data cache misses".
  Slices of 5–15 instructions (per-benchmark means 5.2–34.8), 2.5–6 live-ins. Basic triggers: +13.5 % average on
  8 contexts; chaining triggers +59 % with 4 threads, +76 % with 8; with one helper context mcf and mst +12–13 %.
  mcf: accuracy 98 %, coverage 76 % (basic), 92 % / 88 % (chaining).
- Collins, Tullsen, Wang, Shen, "Dynamic speculative precomputation", MICRO 2001,
  https://cseweb.ucsd.edu/~tullsen/dsp.pdf (body). Slices built by hardware from a 512-entry retired-instruction
  buffer; a load is delinquent if it runs at least 100 times in 128K instructions and blocks commit 4 or more cycles on
  average. +14.3 % basic, +33.7 % with chaining, +5.5 % with two contexts.
- Wang et al., "Helper threads via virtual multithreading on an experimental Itanium 2 processor-based platform",
  ASPLOS 2004 (body,
  https://cs.uwaterloo.ca/~brecht/courses/856-Topics-In-Computer-Systems-2009/Possible-Readings/threads/virtual-multithreading-itanium-asplos-2004.pdf).
  Real hardware, switch on a miss in firmware (70 cycles each way). +5.8–38.5 % (mcf 38.5 %); helpers of 32–50
  instructions; misses removed 48.8 % on average, mcf 83.7 %, vpr 47.9 %.
- Kim et al., "Physical experimentation with prefetching helper threads on Intel's Hyper-Threaded processors", CGO
  2004, http://cgo.org/cgo2004/papers/02_80_Kim_D_REVISED.pdf (body for costs and coverage). Fewer than 5 loads
  cause 83.5 % of L2 misses. Waking a helper through the Windows API costs 10–30K cycles, a prototype hardware
  mechanism 1500. L2 misses removed in the target loop 25.3–60.4 %; whole-program gains single-digit, some losses
  (per-benchmark figure only).
- Zilles & Sohi, "Understanding the backward slices of performance degrading instructions", ISCA 2000,
  https://ftp.cs.wisc.edu/sohi/papers/2000/slice.isca.pdf (body): in the 512 instructions before an L2 miss the
  address slice is 4–10 % of the instructions; speculation shrinks many slices below one tenth. "Execution-based
  prediction using speculative slices", ISCA 2001, https://ftp.cs.wisc.edu/sohi/papers/2001/slice.isca.pdf (body):
  +1–43 %, mostly from branch prediction; slices are 10–15 % of fetched instructions.
- Roth & Sohi, "Speculative data-driven multithreading", HPCA 2001,
  https://ftp.cs.wisc.edu/sohi/papers/2001/ddmt.hpca.pdf (body): 4.1–24.8 % saved, memory at only 70 cycles.
- Luk, "Tolerating memory latency through software-controlled pre-execution in simultaneous multithreading
  processors", ISCA 2001 (abstract): +24 % on irregular programs.

### Helpers on another core, sharing only a cache level: what applies to the A57

- Brown, Wang, Chrysos, Wang, Shen, "Speculative precomputation on chip multiprocessors", MTEAC-6, 2002,
  https://cseweb.ucsd.edu/~jbrown/papers/sp-cmp.pdf (body). In-order Itanium CMP, private L2, shared L3 at 30
  cycles, memory 230. SMT beats every CMP set-up; the cost of talking across cores "far outweighs" the gain from less
  contention; prefetches served by the shared level are "roughly 30 times slower than those satisfied at L1".
  Basic CMP gains about 1.2–1.6× (figure only).
- Lee, Jung, Lim, Solihin, "Prefetching with helper threads for loosely coupled multiprocessor systems", IEEE TPDS
  2009 (body, from a web.archive.org copy of http://www.ece.ncsu.edu/arpers/Papers/tpds09-helper.pdf). Simulated CMP
  1.33×. **Real Intel Core Duo, shared 2 MB L2: 1.15× over the hardware L2 prefetcher on the high-miss programs**
  (bzip2, cg, em3d, mcf, mst, swim); equake, mg and parser slowed down from synchronisation. User-level semaphores in
  shared memory; the helper runs at most MAX_DIST × LOOP_SYNC_INTERVAL iterations ahead and skips forward when it
  falls behind. Model: T_new = T_orig − T_mem × (1 − Miss_new / Miss_orig). If the helper is not much shorter than the
  main loop, it slows the program.
- Kamruzzaman, Swanson, Tullsen, "Inter-core prefetching for multicore processors using migrating helper threads",
  ASPLOS 2011, https://cseweb.ucsd.edu/~swanson/papers/ASPLOS2011Prefetching.pdf (body). Real Core 2 Quad, Nehalem,
  Opteron: +20–50 % untuned, +31–63 % tuned, LBM 2.8×; mcf 1.6× on Opteron, nothing on Nehalem. It works by moving the
  main thread onto the core whose private cache the helper has filled (swap 1.7–2 µs, chunks 128 KB–1 MB), mostly on
  regular NAS and SPEC FP code with hand-written slices.
- Lu et al., "Dynamic helper threaded prefetching on the Sun UltraSPARC CMP processor", MICRO 2005 (not verified, from
  search summaries): up to +35 % on mcf, 2–16 % on Fluent, one helper on a dual core with a shared L2.
- Ganusov & Burtscher, "Efficient emulation of hardware prefetchers via event-driven helper threading", PACT 2006,
  https://userweb.cs.txstate.edu/~mb92/papers/pact06.pdf (body): +20 % with a Markov helper, but it needs a hardware
  buffer that feeds L2-miss events to the other core.
- Solihin, Lee, Torrellas, "Using a user-level memory thread for correlation prefetching", ISCA 2002,
  https://iacoma.cs.uiuc.edu/iacoma-papers/isca02pref.pdf, and TPDS 14(6), 2003 (abstract only, not verified): a
  thread replaying correlated miss addresses gives 1.32× on nine mostly irregular programs, 1.46× with a sequential
  prefetcher. Their thread runs in the memory controller, not on a core.
- Guo et al., "Ghost threading", MICRO 2025, https://www.cl.cam.ac.uk/~tmj32/papers/docs/guo25-micro.pdf (body):
  real Core i7-12700, SMT only, 1.33× against 1.06× for software prefetching. Its throttle (a shared counter checked
  every SYNC_FREQ iterations, TOO_FAR and CLOSE thresholds, skipping ahead when late) carries over to a CMP.

### Runahead and slipstream

- Mutlu, Stark, Wilkerson, Patt, "Runahead execution", HPCA 2003 (body,
  https://cs.uwaterloo.ca/~brecht/courses/854-Scalable-Systems-Software-2011/Possible-Readings/prefetching-to-cache/mutlu_hpca03_runahead_cpu.pdf):
  +22 % IPC at 495-cycle memory; Mutlu, Kim, Patt, ISCA 2005, https://pages.cs.wisc.edu/~isca2005/papers/06B-01.PDF
  (body): +22.6 % IPC for +26.5 % executed instructions, cut to +6.2 %. Precise runahead (HPCA 2020) 1.20×; vector
  runahead (Naithani, Ainsworth, Jones, Eeckhout, ISCA 2021, body) 1.79×. All need hardware; no software runahead in
  this sense exists.
- The software analogues are decoupled access/execute: Clairvoyance (Tran et al., CGO 2017,
  https://www.diva-portal.org/smash/get/diva2:1077941/FULLTEXT01.pdf, body), measured on an eight-core AArch64
  X-Gene: memory-bound geomean +7–13 %, mcf +43 %; Weber et al. on big.LITTLE (arXiv 1701.05478, body): access phase
  on the small cluster, execute IPC up to +37 %, high locking cost.
- Slipstream: Sundaramoorthy, Purser, Rotenberg, ASPLOS 2000 (body,
  https://ericrotenberg.wordpress.ncsu.edu/files/2025/06/paper_ASPLOS-9.pdf): +7 % on SPEC95 integer, new hardware;
  Ibrahim, Byrd, Rotenberg, HPCA 2003 (body, https://ericrotenberg.wordpress.ncsu.edu/files/2025/06/paper_HPCA-9.pdf):
  dual cores with a shared L2, a reduced copy prefetching into it: +12–19 %, on parallel scientific codes, and only
  because the copy skips real work.

### Prefetching in dynamic optimisers and binary rewriters

- ADORE: Lu, Chen, Fu, Hsu, Othmer, Yew, Chen, "The performance of runtime data cache prefetching in a dynamic
  optimization system", MICRO 2003, https://www.microarch.org/micro36/html/pdf/lu-PerformanceRuntimeData.pdf (body).
  Itanium 2; miss PCs and data addresses from the PMU's data event address registers; distance = miss latency / loop
  body cycles; at most three delinquent loads per loop. 9 of 17 SPEC2000 programs +3–57 %, the rest −2 to +1 %,
  overhead 1–2 %.
- Trident: Zhang, Calder, Tullsen, "A self-repairing prefetcher in an event-driven dynamic optimization framework",
  CGO 2006, https://cseweb.ucsd.edu/~calder/papers/CGO-06-SelfRepairPrefetching.pdf (body): +23 % over a hardware
  stride prefetcher; hot traces hold 85 % of misses, about 55 % are prefetchable; needs hardware watch tables.
- UMI: Zhao, Rabbah, Amarasinghe, Rudolph, Wong, "Ubiquitous memory introspection", CGO 2007, on DynamoRIO,
  https://www.burningcutlery.com/derek/docs/zhao-introspection-cgo07.pdf (body). Software only: hot traces are
  instrumented and fed to a small cache simulator; 14 % overhead; finds delinquent loads with 61 % accuracy (88 % on
  high-miss programs); prefetches gave +11 % average in 11 of 32 programs, best +64 %. This is the method that fits a
  translator without a PMU.
- RPG2: Zhang et al., ASPLOS 2024, https://zyuxuan0115.github.io/files/RPG2_ASPLOS_2024.pdf (body): binary patching
  at run time, up to 2.15×, rolls back harmful prefetches; needs Intel PEBS.
- APT-GET: Jamilan, Khan, Ayers, Kasikci, Litz, EuroSys 2022, https://people.ucsc.edu/~hlitz/papers/apt-get.pdf
  (body): distance and site from Intel LBR timing; 1.30× average, 1.25× over fixed distances.
- "Helper without threads": Sankaranarayanan, Lin, Chinya, arXiv 2009.00202, https://arxiv.org/pdf/2009.00202
  (body). Slices inlined into the main loop of binaries; 13–83 % faster than a helper thread, whose start costs 3–30 µs.
  Its trace pool includes games: about 3 prefetchable irregular delinquent loads per trace; removing all their stall
  would give 15 % geomean.
- Ayers, Litz, Kozyrakis, Ranganathan, "Classifying memory access patterns for prefetching", ASPLOS 2020,
  https://people.ucsc.edu/~hlitz/papers/asplos2020.pdf (body): stride-friendly misses are under 20 % in several
  programs; when the address kernel is as long as the program (a pointer chase), prefetching is useless.

No study of helpers or prefetch on game engines was found.

### The Cortex-A57 itself

- Ainsworth & Jones, "Software prefetching for indirect memory accesses", CGO 2017,
  https://www.cl.cam.ac.uk/~tmj32/papers/docs/ainsworth17-cgo.pdf, and TOCS 2019,
  https://www.cl.cam.ac.uk/~tmj32/papers/docs/ainsworth19-tocs.pdf (body). Nvidia TX1, A57 at 1.9 GHz, 32 KB L1D,
  2 MB L2, the Switch's SoC. Averages: A57 1.1×, A53 (in order) 2.1×, Haswell 1.3×. The A57 "can only support one
  page-table walk at a time on a TLB miss"; distance 64 is near-optimal; late is worse than early; prefetch pays on
  out-of-order cores only for long loops on large data. Per-benchmark A57 values are figure only.
- Cortex-A57 Technical Reference Manual r1p3 (DDI0488H),
  https://documentation-service.arm.com/static/5e906b9fc8052b1608760b6b (body). An L1D prefetcher exists
  (`CPUACTLR_EL1[56]` disables it; algorithm not documented). The L2 prefetcher issues 0/2/4/8 prefetches into L2 on an
  L2 miss with a stride match (`CPUECTLR_EL1[33:32]`, reset 8), detects strides only within a 4 KB page (64 KB with
  large pages), has a 10-entry queue per core and at most 12 outstanding; there are separate instruction and
  table-walk prefetchers. `PLD`/`PRFM` L1 forms allocate into the executing core's L1D and "might be dropped"; the TRM
  does not describe `PLDL2KEEP`. TLBs: 32-entry L1 DTLB and a 1024-entry L2 TLB per core.
- Cortex-A57 Software Optimization Guide, https://documentation-service.arm.com/static/5ed75ee1ca06a95ce53f93c0
  (body): `PRFM` uses the single load pipe, latency 4–5, one per cycle; "one load µop and one store µop every cycle".

### How a translator would build a helper slice

The published recipe, in four steps:

1. Find delinquent loads: a profile of misses per load address. Fewer than 5–10 static loads usually cause more than
   80 % of misses (Collins; Kim). Without data-address sampling the profile comes from cache simulation of a trace
   (UMI's method, or `drcachesim` on T4's trace).
2. Pick the region: a hot loop or call with enough work between misses for the helper to get ahead.
3. Extract the backward slice of each load's address within 256–512 instructions; drop stores, cut rare paths,
   speculate through ambiguous memory; the final load becomes a prefetch. 5–50 instructions.
4. Place triggers and pace the helper: a trigger in the main thread passes live-ins; the helper chains itself
   through the loop; a shared counter keeps it between CLOSE and TOO_FAR.

### The numbers for this game

Inputs: 67 % of the main thread's samples on loads and stores (stack 20 %, other memory 48 %), L1 4 cycles, L2 34,
DRAM 377, TLB miss 356 (on top of nothing: the walk's own lines are cached), one frame about 41.9 M cycles at 45 fps.

- What the samples say: a sample lands on the oldest unfinished instruction. Stack accesses are almost all L1 hits
  or store-forwarded (6.5–7 cycles, `[A57]`). The 48 % on other memory is an upper bound for L1 + L2 + DRAM stalls
  together, and a dependent L1 hit already costs 4 cycles on a chain. So m (DRAM stall share) is somewhere between a
  few percent and about 35 %; 10–30 % is the working range.
- What a helper saves: only DRAM misses, each by at most 343 of 377 cycles; and only if it gets 377 or more cycles
  ahead, which needs a slice much shorter than the main thread's path to the load (Lee/Solihin).
- Coverage c: 25–60 % in the measured cases (Kim; Wang), 76–92 % only for mcf-like loops with chaining triggers.

| m | c = 0.3 | c = 0.45 | c = 0.6 |
|---|---|---|---|
| 0.10 | 1.03 | 1.04 | 1.06 |
| 0.20 | 1.06 | 1.09 | 1.12 |
| 0.30 | 1.09 | 1.14 | 1.20 |

- The cost: a helper on a free core (1.5 of 4 are in use, more with W1's worker); LPDDR4 bandwidth shared with the
  GPU, so useless prefetches cost the GPU; the main thread's trigger stores, 5–10 instructions per trigger.

Why a game fits frame correlation better than slices: the main thread repeats the same work each frame (the same
effect passes, the same D3DX state records of 0x64 bytes, the same scene nodes, mostly the same order). A helper that
replays the miss stream of frame N in frame N+1, paced by a progress counter, needs no slice and no live-ins; the
risk is order changes (culling, camera), which cost useless prefetches, not wrong results. A prefetch never changes
program results, so no speculation hardware is needed. This is Solihin's correlation prefetching run on a spare core
(not verified in body).

## What this means for the orchestrator

A component "memory helper", built only if R1 shows m ≥ 0.15:

1. **Profile in the lab** (`tools/analysis/`): T4's drmemtrace trace through `drcachesim` set to the A57 (32 KB 2-way
   L1D, 2 MB 16-way shared L2, 64-byte lines, with a stride prefetcher model). Output per guest load address: misses,
   L2 hits, DRAM misses, and whether the misses repeat frame to frame (same lines, same order). The top loads, with
   their frame-relative miss lists, go into the per-game profile.
2. **Two mechanisms, by what the profile shows:**
   - Strided or short-distance misses inside a hot loop: FEX emits `PRFM PLDL1KEEP` with a distance in the loop body
     for those guest addresses (W7). It is the only way to hide L2 hits too. Expected small (1.1× on this core in
     Ainsworth & Jones).
   - Repeating misses across frames: a helper thread replays the previous frame's DRAM-miss lines in order, with
     `PRFM PLDL2KEEP` (or plain loads where the PRFM form is dropped, R4). Pacing: the main thread's count of Direct3D
     calls in this frame (the W1 proxy already counts; without it DXVK's `0005` counters) is the progress clock; the
     helper stays 1–4 draws ahead. The miss list of the current frame is refreshed by sampled FEX instrumentation of the
     profiled loads only (one store of the address to a ring per sampled access).
3. **Placement:** the helper runs on a core with room, below the game and W1's worker in priority, and sleeps when
   it is behind or when the frame's list is done. The orchestrator turns it off when frame time does not improve
   (closed loop, W12).
4. **Not built:** backward-slice helpers compiled by FEX at run time. Every real-hardware result with only a shared
   cache level is small, and the slice must be shorter than the main thread's path, which in game code full of calls
   and x87 is unlikely.

## Needs the console or the lab

| Name | How | Decision it switches |
|---|---|---|
| R1 DRAM stall share m | lab: `drcachesim` over T4's trace with the A57's caches gives DRAM misses per frame; m ≤ misses × 377 / cycles per frame (an upper bound, no overlap) | m < 0.10: no helper; 0.10–0.20: inline `PRFM` only; > 0.20: try the frame-replay helper |
| R2 frame repeatability | lab: from the same trace, the share of frame N's DRAM-missing lines that miss again in frame N+1, and how close their order is | whether the frame-replay helper can cover c ≥ 0.4 |
| R3 L2 sensitivity | console: a co-runner thread on another core keeps 0, 0.5, 1, 1.5 MB of its own buffer hot in L2; frame time of the race against the stolen capacity | how much the main thread depends on L2 residency; a check on R1 without a PMU |
| R4 `PRFM` forms | `[A57]`: dependent-load latency after `PRFM PLDL1KEEP`, `PLDL2KEEP`, `PLDL2STRM` and after a plain load from another core, to the same line (console) | which instruction the helper and FEX emit; whether a prefetch from another core lands in L2 |
| R5 helper wake and pace cost | `[A57]`: latency for a counter written on one core to be seen on another (also C3, S5) | the helper's lead in draws; polling or sleeping |
| R6 helper's effect | console, behind an Experimental row: race frame time and `[CLOCKS]` GPU busy with the helper on and off | keep or drop the helper |
