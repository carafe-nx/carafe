# Orchestrator: what the research found and how the system is built

The brief is [REQUIREMENTS.md](REQUIREMENTS.md). Each question has its finding in `findings/`, linked from
[QUESTIONS.md](QUESTIONS.md); the one console run that closes the open numbers is [CONSOLE-CHECKS.md](CONSOLE-CHECKS.md).
Sources were read at: Atmosphère `6b0c077`, Autorun `636913ca` (and `a54d5964` for what changed upstream), FEX
`395b132` with Carafe's patches, DXVK `b1a1c99`, Mesa for the Switch `d4a00ea0ab` (danfromtico/mesa-switch), Eden
`58c1e20`, and ARM's Cortex-A57 Software Optimization Guide (UAN 0015B). Console data are the October 2026 runs of
Need for Speed Underground 2 at 1887 MHz (`nfsu2-exp5`, `-exp7`, `-exp9`). Nothing here was timed on the console
beyond those runs; every number that still needs it is a range with its condition.

## 1. The answer in one page

The frame of a translated Direct3D 9 game is one producer of graphics commands running on one core. In NFSU2's race
the main thread is 94–99 % busy and waits for nothing; every other thread waits for it. The GPU finishes a frame in
9–16 ms and then idles until the next one, so it is 46–73 % busy; its own ceiling is 61–107 fps, median 83. The other
three cores are 60–95 % free. The console is therefore limited by how much work sits on one thread, not by any shared
resource: DRAM bandwidth is barely used, the GPU is starved, the cores are idle.

Where the main thread's time goes (`findings/frame-stages.md`, `findings/main-thread-memory-stalls.md`):

| Part of the frame | Share | What it is |
|---|---|---|
| D3DX effect application (linked into the `.exe`) | 30–35 % (at least 27 %) | every pass re-sends all its states and re-evaluates every state expression |
| DXVK's Direct3D 9 front end | 17.5 % | validation, state copy, `PrepareDraw`, constant copies, command recording |
| the game's render code | 12–25 % | scene walk, visibility tests, draw lists |
| D3DX x87 math and C runtime helpers | 6–7 % | |
| simulation and updates | 3–15 % | the race world and its cars |
| Wine and system calls | 3.5 % | |
| not attributed | about 13 % | outside the 256 dumped blocks |

How the time is spent inside, by what the core waits on: about 28 % on the instruction after a branch (mispredicts
and instruction fetch; 194 KB of hot code against a 48 KB instruction cache), 45–55 % on L1-latency chains that
translation shapes (x86 stack slots, FEX's x87 and lookup state, object chains that hit L1), 5–15 % on L2, and only
3–7 % on DRAM, most of it two loads in one visibility test.

So the orchestrator wins in three ways, in this order of size:

1. **Take the graphics command producer off the main thread.** D3DX effects and the Direct3D 9 front end are half of
   the thread, a one-way stream of commands that returns almost nothing to the game. Make that work cheaper where it
   is wasted (D3DX re-sends unchanged state) and move the rest to a free core.
2. **Make what stays cheaper to run.** FEX's block edges, returns, indirect calls and code layout; the SSE matrix
   multiply that D3DX does not pick; large pages; a few targeted prefetches.
3. **Own the cores.** Place every thread by role, pin the main thread alone, give each core its tenants, map Windows
   priorities, and measure everything it does in the same report lines the profiler already writes.

Estimated together, on NFSU2's profile: the main thread needs 0.44–0.70 of today's time, 1.4–2.3× faster. That
reaches the GPU's ceiling in heavy scenes at the low end and everywhere at the high end; the frame then caps at the
screen's 60 fps or at the GPU. Section 5 shows the arithmetic.

## 2. The console as the orchestrator sees it

### Cores and the scheduler

- The kernel is mesosphère itself: Atmosphère boots it in place of Nintendo's kernel since 1.2.0
  (`findings/horizon-scheduler.md`). Scheduling is strict priority (0 highest, 63 lowest), one run queue per core, one
  global scheduler lock.
- Time is shared at one priority per core only: every 10 ms a kernel thread rotates priority 59 on cores 0–2 and 63
  on core 3 (`libmesosphere/source/kern_k_dpc_manager.cpp:103-146`, checked). Any other priority runs until it blocks.
  A thread woken at 59 on a busy core waits up to 10 ms per thread ahead of it.
- A thread runs only where its core mask allows; with a mask wider than one core the kernel moves runnable threads to
  idle cores at every scheduler update.
- Kernel mutexes lend priority to the holder; condition variables and address waits do not.
- Windows thread priorities never reach Horizon: Wine's server only stores them (`dlls/ntdll/unix/horizon.c:11911`,
  checked) and every program thread runs at 59 (`horizon-wine/source/runtime.c:2350-2356`). Autorun sets its own
  threads: server 57, audio 56, graphics workers on core 3 at 63.
- Carafe's NSP grants cores 0–3 and priorities 28–63 (`runtime/loader/carafe-loader.json:30-33`, checked), so core 3
  is available in every Carafe game, with or without Autorun's forwarder.

### Core 3

Every system module starts there and most may use no other core; their threads run at priorities 15–58, device
interrupts land there too (`findings/core-3.md`). A thread at 63 gets what they leave. In the race `dxvk-cs` used
35–40 % of core 3 and was runnable but not running about 22 % of its time (`findings/gpu-submission.md`): the system
takes a real share. Core 3 is a scavenger core for work that tolerates millisecond delays (compiling, prefetch,
analysis), not for a stage the main thread waits on. Owning more of it would mean preempting input, file, GPU and
display services; rejected.

### Memory and caches

| | Value | Source |
|---|---|---|
| L1D / L2 / DRAM latency | 4 / 34 / 377 cycles | `[A57]`, console |
| TLB reach, L1 / L2 | 128 KB / 4 MB with 4 KB pages; 2 MB / 64 MB with 64 KB pages | `findings/memory-pages.md` |
| page-walk cost | unknown: the `[A57]` "page" test measured DRAM, its buffer is 2 MB-mapped | `findings/memory-pages.md` |
| DRAM bandwidth | 21.3 GB/s handheld, 25.6 GB/s docked; GPU uses 3–8 GB/s, the main thread under 1.3 | `findings/memory-bandwidth.md` |
| line hand-off between cores | 34 cycles measured through L2, 40–100 estimated when dirty | `findings/horizon-scheduler.md` |
| wake of a thread on another idle core | 1.5–5 µs estimated | `findings/horizon-scheduler.md` |

- The kernel picks page sizes from address alignment alone. The heap and FEX's code arenas get large entries; the
  runtime image (82 MB at `0x2186000`) and most guest data get 4 KB entries.
- Every buffer DXVK maps for the CPU is uncached: NVK exposes a cached non-coherent type and a coherent uncached type,
  DXVK always asks for coherent (`nfsu2-exp9/logs/autorun_runtime.log:442-445`, checked). Writes in whole lines are
  cheap; reads cost DRAM each (`findings/shared-memory-attributes.md`).
- The A57's performance counters are open to user mode when the user sets `enable_user_pmu_access=1` in
  `sdmc:/exosphere.ini` (`libmesosphere/source/arch/arm64/kern_cpu.cpp:518`, checked). Off by default.

### The pipeline today

main thread (game, D3DX, DXVK front end; core 1) → `dxvk-cs` (Vulkan through NVK; core 3, 31–40 % busy, idle 31–57 %)
→ `dxvk-submit` (core 2) → GPU → `dxvk-queue` (core 0, waits on the fence). Every arrow is one-way; the main thread's
only wait is 0.4 % inside DXVK (`findings/threads-in-a-race.md`). After the main thread, `dxvk-cs` is the next limit,
at about 80–100 fps.

## 3. The design

The orchestrator is one part of the runtime with a small core and mechanisms in the layers it steers. Its code lives
in `runtime/orchestrator/`, built into `wine-nx-runtime.elf` the way the overlay is; what it changes in FEX, DXVK and
Autorun stays patches in `runtime/patches/`. Nothing needs Autorun's design changed (`findings/autorun-boundaries.md`).

```
                       ┌──────────────────────── orchestrator core (runtime/orchestrator) ────────────────────────┐
  game start ─────────►│ plan: game facts from the NSP (Direct3D version, static D3DX release and sites,           │
                       │       previous run's profile), core map (core 3 or not, PMU or not)                      │
                       │ observe (every report, ~1 s): threads, per-core idle, waits, frame time, GPU busy,       │
                       │       FEX and DXVK counters, PMU sets when open                                          │
                       │ decide: placement and priorities, queue policy, compile priority, when to swap tiers    │
                       │ act: core masks and priorities through Horizon, flags in a shared page for FEX and DXVK │
                       └───────┬────────────────────┬──────────────────────┬─────────────────────┬────────────────┘
                               │                    │                      │                     │
                  threads (Horizon)      graphics front end      translation (FEX)        memory and I/O
                  roles, cores, 58/59    proxy d3d9 + effect     block edges, tiers,      large pages, cached
                  pinned main thread     queue on a free core    compile service          staging, SD prefetch
```

### 3.1 Core: plan, observe, decide, act

- **Plan at start.** From the NSP (Carafe's builder scans the `.exe`): the Direct3D version, whether D3DX is linked in,
  its release and the sites of its dispatch table and effect vtable (`findings/d3dx-in-exe.md`). From the SD card: the
  previous run's profile beside the disk cache (`findings/closed-loop-profile.md`). From the process: core masks (is
  core 3 there: `wine_nx_four_cores_available`), whether `PMUSERENR_EL0` is set.
- **Observe.** Per report, from memory it already shares: registered threads with busy share and core, every native
  thread too (wrap libnx thread creation), per-core idle ticks (`svcGetInfo`, idle tick count), main-thread waits by
  kind (wrapped `horizon_futex_wait`, `horizon_server_sync_call`), `[FPS]`, `[CLOCKS]` GPU busy, `dxvk-queue`'s
  fence-wait share (the same measure without the ioctl), FEX's counters put in process memory, the proxy's counters,
  PMU set A/B when open. The cost must stay near zero: counters, not tracing.
- **Decide, slowly.** Seconds, not frames. The first rule is the classification already in the logs: main thread at
  95 % or more with GPU busy well under 90 % is CPU-bound, and fps ÷ busy is the headroom any CPU-side change can buy.
- **Act.** Core masks and priorities through `__real_svcSetThreadCoreMask` and `svcSetThreadPriority`; flags and
  limits in a shared page FEX and DXVK read with no unix call; nothing that changes code under the game thread except
  through FEX's own invalidation path.

### 3.2 Threads and cores

| Role | Core | Priority | Why |
|---|---|---|---|
| main thread | one core alone (core 1), mask of that core only | 58, pinned; every wait in it must block, not spin | at 59 any other 59 thread on the core takes 10 ms slices; Autorun keeps 59 because a spinning 58 starved a 59 holder (`runtime.c:2350-2353`), which the wait wrappers must rule out |
| graphics front-end worker (3.3) | the free core of 0 and 2, mask 0x5 | 58–59 | the main thread waits for it at sync points and at the queue bound, so it must not sit behind the system on core 3 |
| `dxvk-cs` | core 3 at 63 today; to cores 0/2 when its runnable-but-waiting time grows | 59 there | it is the next limit after the main thread |
| `dxvk-submit`, `dxvk-queue` | 0/2 | 59 | short, mostly waiting |
| server threads, audio | as Autorun sets them (57, 56) | | latency-bound, short |
| compile service, prefetch helper, profile analysis | core 3 | 63 | tolerate delay; the L2 is shared, so prefetches from core 3 serve core 1 |
| game threads with a Windows priority | mapped to 58–60 around 59 | | keeps the 59 rotation, gives the game's intent a meaning |

Without core 3 the same table runs on three cores: the main thread alone on one, everything else on the other two
with mask 0x6.

Rules for any new hand-off: batch so that one wake carries tens of microseconds of work, wake only on empty to
non-empty, keep producer and consumer indices on separate cache lines, and give shared objects one owning core (the
main thread and `dxvk-cs` already pay 100–135 cycles per reference-count atomic on shared lines).

### 3.3 The graphics front end off the main thread

A proxy `d3d9.dll` (i386, under FEX like DXVK) between the game and DXVK, installed by `runtime/dxvk/use-dxvk.py`
with DXVK renamed beside it (`findings/d3d9-front-end.md`, `findings/cpu-graphics-split.md`). The cloud study's
skeleton (`D:\debug_switch\carafe\cloud\results-native-d3d9`) is the start.

- **On the game thread:** a shadow of all device state; unchanged setters return at once; `Get*` answered from the
  shadow; everything else appended to a single-producer ring with inline payloads (matrices, constants, vertex data of
  draws from memory). State-block applies are mirrored into the shadow and queued, never synchronous.
- **Locks:** `DISCARD` and `NOOVERWRITE` answered from the proxy's own staging with a copy queued at `Unlock`;
  write-only locks renamed the same way; reads drain the ring. Queries return `S_FALSE` until the worker has the
  result, which Direct3D 9 allows.
- **The worker** replays the ring into today's DXVK; `dxvk-cs` stays behind it. The ring is bounded at two `Present`s
  so input latency does not grow; streaming in chunks costs 1–2 ms and the shorter frames more than repay it
  (`findings/frame-pipelining.md`).
- **Level 2:** the D3DX effect calls go into the same ring, so the effect framework's work (pass application, state
  expressions) runs on the worker too. The proxy patches the vtable of effect objects the linked D3DX creates, found
  by the scan of 3.1; static answers (pass counts, handles) come from a cache filled on first use.
- **Policy, the orchestrator's:** level per game from counters; the worker's core and spin length from the measured
  wake cost; the proxy off for a game whose drains per frame pass a threshold.

### 3.4 Less work where it is wasted: D3DX

The static D3DX in games of 2003 to early 2006 is found by markers that do not depend on the game: the `cpuid`
dispatch routine and its table writes, the effect compiler's error strings, the shader compiler version strings
(found alike in NFSU and NFSU2). Later games import `d3dx9_24`–`d3dx9_43`, which Autorun serves from Wine's builtin.

1. Point the dispatch table's matrix-multiply slot at an SSE function: about 3 % of the main thread, low risk.
2. Drop unchanged states where D3DX's own loop sends them, before the indirect call to the device; safe only with the
   proxy's shadow (3.3).
3. Skip D3DX's per-state work: unchanged static states and expressions whose inputs did not change, as Wine's
   `update_version` does. 1.15–1.25× on the main thread; needs the internals of each static D3DX release (about
   seven exist), so it is built per release, not per game.
4. With level 2 the rest of the effect work leaves the main thread.

For games with the D3DX DLL the same filter goes into Wine's `d3dx9_36` build.

### 3.5 Translation for the A57

Instruction choice by A57 cost is nearly spent (under 1 % left). What FEX adds now sits at block edges
(`findings/a57-translation.md`):

| Change | Main thread | Needs |
|---|---|---|
| monomorphic inline caches at indirect calls | 2–3 % | FEX backend |
| keep the call-return stack in step at the system-call bridge (the dropped `SyscallReturnStack` patch) | 1–2.5 % | the missed-return measurement first |
| MSVC jump tables in `.text` as branches inside the multiblock | 1–2 % | FEX frontend |
| slimmer block entries | 1–3 % | FEX backend |
| code layout for the 48 KB L1I: cold paths out of line, hot blocks packed, targets on 16 bytes | 0–8 % | the profile, the compile service |
| profile-guided second tier: superblocks through calls, x87 stack and registers kept across edges | 2–5 % | the profile, the compile service |
| a base register for module data | 1–3 % | FEX backend |
| `PRFM` at the few DRAM-class sites (the visibility test's two loads; D3DX record chains) | 1–3 % | block counts or the lab trace |

Together 8–15 % without double counting. The **compile service** runs on core 3 at 63: one guest thread entering FEX
through its own entry point, compiling the previous run's list ahead, the exits of each new block, and the second
tier when idle; with the disk cache on, it also takes the cache's reads and stores off the game thread
(`findings/translate-ahead.md`). It does not raise the race's average frame rate (compiling is under 0.5 % there); it
removes compile hitches and shortens start-up by seconds.

### 3.6 Memory and I/O

- **Large pages:** map the runtime image congruent with the heap modulo 2 MB in Carafe's loader
  (`runtime/loader/source/main.c:401`); give guest reservations of 2 MB and more 2 MB-aligned addresses and sources in
  Wine's mapping code. 1–6 % if the data overflows the 4 MB TLB reach (`findings/memory-pages.md`).
- **Cached staging for games that read back:** a DXVK patch giving staging and non-write-only buffers the cached type
  with flush and invalidate at the dirty range. Zero for games that do not read back; large for those that do.
- **NVK push buffers:** `NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=0/1` is an A/B for `dxvk-cs`, set before the device is made.
- **SD card:** race reads are off the main thread (frame rate unchanged); a recorded read profile replayed as prefetch
  hides 0.7–1.4 s of card waits per load (`findings/sd-read-ahead.md`).

### 3.7 Games of other shapes

- **Several busy threads.** The rule "the main thread alone on its core" holds only when one thread dominates
  (90 % or more of a core while the others are small). With several busy game threads, the orchestrator spreads them
  over cores 0–2 with wide masks so the kernel's idle-core stealing places them, and keeps everything of its own off
  their cores.
- **Priorities above 59** go only to threads known to block in their waits. A thread at 58 that spins on a lock held
  by a 59 thread on the same core never lets the holder run (`runtime.c:2350-2353`); the wait wrappers tell which
  threads spin. No two game threads at different priorities share a core.
- **Direct3D 9 from several threads** (`D3DCREATE_MULTITHREADED`): the proxy's ring takes records from several
  producers in the device lock's order, or the proxy stays off for that device; the flag is seen at device creation.
  Direct3D 10 and 11 games are not proxied; DXVK is already multithreaded there, and they get placement and
  priorities.
- **Games whose logic is heavy** (strategy, simulation): the stage tag (P5) tells a "drawing" game from a "thinking"
  one. Moving graphics gains less there, and splitting the game's own logic becomes the first lever.

### 3.8 The game's own logic

After the stages, the main thread keeps the game's own logic and render code, 44–50 % of today's time in NFSU2,
on a core of its own. Two ways can split it further, both decided by measurements, not dropped:

- **A frame pipeline:** the simulation of frame N+1 alongside the render of frame N, on a snapshot of the memory the
  render reads. Bound 1.5–1.7×; pays if the game writes no more than about 300 pages a frame (L1 and P5 of
  CONSOLE-CHECKS).
- **Independent loops across cores,** such as the visibility tests of the scene walk, chosen by the dependence limit
  (L2).

They come after the graphics stages, which take the larger share with less risk, unless the stage tag shows a game
whose logic dominates.

## 4. What was weighed and left

| Way | Gain here | Why it is not in the plan |
|---|---|---|
| a memory run-ahead or helper thread (W5) | 1.03–1.07× at most | DRAM is 3–7 % of the main thread; a helper cannot hide L1 chains or fill the main core's TLB; targeted `PRFM` reaches the same sites without a thread |
| speculative execution in FEX (W6) | 1.00–1.05× generic, 1.04–1.14× for chosen loops | no software scheme has sped up irregular code on cores 75–110 cycles apart (HELIX-RC, Janus); the per-loop mode waits for the limit study |
| splitting the game's own frame by snapshot (W2) | bound 1.5–1.7× | pays only below about 300 written pages a frame; a measurement, not a build |
| CPU work on the GPU (W9) | 1–3 % | everything with a GPU form is already there in DXVK; the GPU's idle time is better kept as headroom |
| more of core 3 for the game | — | preempts input, file, GPU and display services |
| native ARM64 D3DX or `d3d9.dll` behind thunks | — | a crossing costs as much as the short calls it replaces; only long functions such as `ApplyPass` could pay |
| leaving Autorun | 0 | every control point is reachable; reconsider only if the server lock costs over 1 % of the main thread, a game runs out of threads, or a rebase costs over a week |

## 5. The gain, worked out

Shares of today's main thread (section 1): D3DX effects E, the Direct3D 9 front end D = 0.175, everything else G.

| Step | Low | High |
|---|---|---|
| start | E 0.30, D 0.175, G 0.525 | same |
| SSE matrix multiply (3.4.1) | G − 0.02 | G − 0.02 |
| D3DX skips unchanged work (3.4.3) | E × 0.65 = 0.195 | E × 0.5 = 0.15 |
| front end off the thread (3.3) | level 1: D × 0.4 = 0.07 | level 2: (E + D) × 0.1 = 0.033 |
| translation (3.5) on what stays | − 8 % of G + E: − 0.056 | − 15 % of G: − 0.076 |
| pages and prefetch (3.5, 3.6) | − 2 %: − 0.014 | − 5 % of G: − 0.025 |
| **main thread** | **0.70 → 1.43×** | **0.44 → 2.29×** |

At today's race median of 47 fps: 67 fps at the low end, 107 at the high end, before the caps. The caps are the GPU
(61–107 fps by scene, median 83), the screen's 60 fps, and `dxvk-cs` at about 80–100 fps, which the plan moves off
core 3 when it nears its limit. So the low end makes heavy scenes GPU-bound and the rest run near 60; the high end
makes the race GPU-bound or 60 fps throughout. The worker of level 2 then carries 0.6–0.7 of a core; cores 0 and 2
have it.

What can move these numbers: the share of D3DX's work that is truly skippable (counted per pass in the lab), the
drains per frame of the proxy (counted), the wake cost (console), the code-layout share (console), and whether other
games have NFSU2's shape. Games without static D3DX lose step 3.4 and keep the rest.

## 6. Stages

Each stage ships behind its own Experimental row, off by default, and is checked in the lab for correctness before the
console; the lab cannot time the A57. The final console run measures [CONSOLE-CHECKS.md](CONSOLE-CHECKS.md) and stage 1
together.

1. **Measurements and the core.** The checks of CONSOLE-CHECKS in `[A57]`, the profiler and DXVK's counters; the
   orchestrator core with every thread registered, per-core idle, wait timing, the placement table of 3.2 and the
   Windows priority mapping; the SSE matrix-multiply slot. Small gain, all the data the next stages need.
2. **Graphics front end, level 1**, with the D3DX filter of 3.4.2 and the sync counters.
3. **D3DX, per release:** skip unchanged work (3.4.3), then level 2.
4. **Translation:** block edges, inline caches and jump tables first (no profile needed); then the per-game profile,
   the compile service, the second tier, code layout and targeted `PRFM`.
5. **Memory:** large pages, cached staging for games that read back, the NVK push-buffer choice.

Order holds unless a check moves it: if drains per frame are high, stage 2 needs renaming and deferred readback
first; if the code-layout share is large, layout moves up in stage 4; if the PMU or the lab trace shows more DRAM time
than the 3–7 % estimated, targeted prefetch moves up.

## 7. Choices that are the owner's

- **The PMU switch.** `enable_user_pmu_access=1` in the user's `exosphere.ini` needs a reboot and changes the user's
  CFW settings. The orchestrator works without it; with it, the console run gives exact cache and branch counts.
- **The limit study's download.** DynamoRIO on the PC would record the main thread's accesses and give the dependence
  limit of parallelism and miss rates offline (`findings/speculative-parallelism.md`); the lab trace through Eden's
  memory callbacks gives miss rates without a download.
- **Experimental rows.** Each stage's row and its default.
