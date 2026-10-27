# Game speed: where CPU time goes in a translated game and how to win it back

A source review, not a measurement. It covers 32-bit Windows games, which run on the console under FEX (WoW64), with
Direct3D 9 through DXVK. FEX was read at `395b132f` (FEX-2609) with Autorun's `horizon-wine/fex/horizon.patch` at
`636913ca` and Carafe's `runtime/patches/fex`, in the patched tree `runtime/build/fex-src`. DXVK was read at
`b1a1c99a` (3.1.1). Lines marked "checked" were read a second time; the rest come from one reading pass and are not
verified. Nothing below was timed on a Cortex-A57.

## Measured and counted, October 2026

The section after this one is the original review. This one adds the console profile of Need for Speed Underground 2
in a race and ARM64 instruction counts from FEX's own `CodeSizeValidation`, built on x86-64 with FEX's simulator and
set up as a Cortex-A57 (no FlagM, AFP, LSE, LRCPC). Counts are not time: every gain below assumes time follows the
number of ARM64 instructions.

Measured on the console:

- The main thread uses 93–95 % of one core, alone on it; `dxvk-cs` uses about 35 % of another and waits half of it.
  The game builds every frame on that one thread, and no community mod changes it: WidescreenFix's
  `SingleCoreAffinity` only keeps the game's own threads on one core to avoid a crash.
- Of the main thread, 88–91 % is translated x86 code: the game about 80 %, `d3d9.dll` 10–12 %. FEX's C++ is under 1 %,
  JIT code outside blocks (dispatcher, block entries) about 5 %, system calls about 2 %.
- `NtQueryPerformanceCounter` runs 1.25 million times a second while loading (a wait loop) and about 5000 in a race.

Counted from the code FEX emits (cloud studies, patches not yet in `runtime/patches`):

- FEX already emits 1.4–3.2 ARM64 instructions per x86 one for integer code and 3.2–5.8 for x87 code. Over the
  game's hot code it is 2.5; a perfect translator could give at most about 1.9×.
- x87 is 32 % of the hot x86 instructions and 34 % of the ARM64 they become. In FEX's real-game x87 test blocks
  `fcvt` is 21–31 % of all ARM64: most operands are float32 and reduced precision keeps doubles, while DXVK sets the
  x87 precision to 24 bits (`src/d3d9/d3d9_device.cpp:95-96, 5924-5943`). Two FEX options written in the study,
  `X87FastStack` (earlier blocks' registers loaded on demand, `fnstsw` reads only TOP, C0–C3 reused) and
  `X87SinglePrecision` (x87 values as float32), cut those blocks 1.10× and 1.24×, together 1.39×.
- About 25 % of the main thread runs under the D3DX9 effect framework linked into the game's executable, which
  re-applies an estimated 15000 render states a frame; DXVK adds 11 %.
- The missing FlagM costs 2–4 %; patches that let the dead-flag pass drop the carry fix-up on ARMv8.0 recover it.

### How fast the A57 is for this game

Clock alone says little. Speed is work per cycle times cycles per second, and the two cores differ most in work per
cycle:

- The Pentium 4 the game lists as its minimum (1 GHz) does little per cycle: a long pipeline flushed on every
  mispredicted branch. The Cortex-A57 is a three-wide out-of-order core from 2015 with good branch prediction and
  larger caches; per cycle it does about 1.5–2× a Pentium 4 (order of magnitude from public benchmarks, not measured
  here). On native code an A57 at 1020 MHz is roughly a Pentium 4 at 1.5–2 GHz.
- Translation costs 2.5 ARM64 instructions per x86 one on this game's hot code, but ARM64 instructions are simpler
  and the A57 runs several a cycle, so in time it costs about 1.5–2×, not 2.5×.
- Together the game sees roughly a Pentium 4 at 0.8–1.3 GHz: its minimum. That matches the 35–46 fps measured.

What follows from it:

- The core is not weak; it delivers what it has. The limit is one thread whose time is spread evenly, with no single
  block of wasted work several times larger than the rest. The largest such block found is D3DX re-applying effects
  (22 %), and most of it is the game's own behaviour.
- Because the A57 does a lot per cycle when its pipeline flows, what slows it is the kind of instruction, not only
  their number: long dependency chains (load → `fcvt` → multiply, each waiting for the last), system-register
  writes such as `msr nzcv`, and moves between general and FP registers. If these stall it, the x87 and flag patches
  win more than their instruction counts say, and counting understates the gain.
- So instruction counts are a floor and a direction, not a forecast. The next step is to measure on the console what
  each kind of instruction costs (a start-up microbenchmark of `fcvt`, `msr`/`mrs nzcv`, GPR↔FP moves, load latency
  and a dependent x87-style chain) and the frame time with each patch, and only then rank the remaining work.

### Direct3D 9 and D3DX

- D3DX's effect code takes at least 22 % of the main thread, and more than half of that is D3DX's own code, not DXVK.
  Every pass the game applies re-sends all of its states to the device with no check and re-evaluates every state
  expression, on every draw. Call chain in SPEED2.EXE: game `0x1cf78b` → Pass (calls ApplyPass at `0x26adb0`) →
  ApplyPass `0x2691a1`, state records of 0x64 bytes. Wine's own `d3dx9_36` skips unchanged parameters with
  `update_version` (`dlls/d3dx9_36/effect.c:1139-1199`); the D3DX the game links has no such check.
- DXVK already drops unchanged values, but its unchanged-value path in `SetRenderState` costs 51 ARM64 instructions
  (63 with the call and return) because the check comes after the device lock. Returning before the lock cuts it to
  25; `SetSamplerState` goes from 48 to 41.
- `D3DXMatrixMultiply` runs D3DX's x87 version: 1616 ARM64 instructions a call against 74 for an SSE version written
  for comparison. It is about 3 % of the main thread (a lower bound: only the top buckets were logged).
- The D3DX linked into SPEED2.EXE fills a table of math functions at start-up (`0x6713a1`) from its own `cpuid` check
  (`0x6712bb`: MMX, SSE, 3DNow!). x87 versions are the default (`0x671aa3`); 3DNow! replaces almost the whole table,
  `D3DXMatrixMultiply` included; SSE replaces only eight entries at the end, and that one is not among them. FEX reports
  SSE, so those eight already run on SSE; this D3DX has no SSE matrix multiply to switch to.
- FEX implements 3DNow! but hides it on Windows (`HostFeatures.cpp`: a black screen in some old EA games with native
  d3dx9). `OverrideFeatures` runs after that, so `FEX_HOSTFEATURES=enable3dnow` reports it again without changing FEX.
  On the console this game took the 3DNow! table without a black screen, but the gain was about 0.5 % of the main
  thread, and the option was dropped (below).
- Removing all of the effect overhead is out of reach while the game calls Pass for every draw.
- With the fixed profiler tables `d3d9.dll` is 19 % of the main thread, not 10–12 %. A d3d9 front end that queues the
  calls for a worker thread on a free core and keeps 22–45 % of today's cost on the game thread would then give
  1.12–1.17×. It pays only if unchanged states are dropped before they are queued: queuing a call costs about 54
  ARM64 instructions, more than the 25 of DXVK's early-out, and in the menus 98–100 % of the state calls change
  nothing. A thunk generator and a skeleton with a worker-thread variant were written and pass a round-trip test
  (cloud study `native-d3d9`); the race's `[D3D9 calls]` decide whether to build it.
- A native ARM64 `d3d9.dll` behind a synchronous 32-bit thunk saves nothing: the thunk costs as much as the cheap
  setters it replaces.

### x87 in detail

- Inside one block FEX's x87 code is close to the guest: 1–3 ARM64 per x87 instruction. The cost sits at block edges
  and at `fnstsw`: a block that only pushes and pops pays 8 instructions for the tag word at its end, each register
  left on the stack 4–6 more, and `fcomp m32; fnstsw ax; test ah, imm; jcc` costs 55 (65 counting its block end).
- With `X87FastStack` that compare costs 38; a fused compare-and-branch would bring it near 21.
- In D3DX's matrix multiply the stack across blocks matters most: its inner loop goes from 69 to 39 ARM64 per
  iteration with the stack kept in registers, 34 with float32 as well.
- `X87SinglePrecision` cuts `fcvt` from 4922 to 410 in FEX's real-game test blocks; it computes in float32, which is
  what Direct3D 9's 24-bit precision control asks for, and differs only in exponent range.

### Other counted costs

- 9–13 % of all ARM64 instructions are `mov`/`movk` pairs forming 32-bit addresses of the game's global variables;
  reusing a nearby constant saves only 1.2 %. A register holding the base of the data section could save one
  instruction per access (estimated 3–6 %, not counted).
- `jit` time is FEX's dispatcher (Autorun leaves the L2 lookup cache off, so an L1 miss spills and calls C++), the
  first two instructions of each block, and the x87 transcendental helpers.
- FEX records only instructions that can fault in a block's address map unless `FEX_GDBSYMBOLS=1`; register-only
  instructions (`fcom st1`, `fnstsw`, `test ah`) are then charged to the instruction before them. A compile that
  restarts keeps the first attempt's map entries.

### Directions and status

Directions for this game, by share of the main thread (estimated): x87 fixes 10–25 %; Direct3D 9 work (D3DX effects
and DXVK) moved off the main thread or made native 9–13 %, a native `d3d9.dll` alone at most 1.10–1.14×; cheaper
block links about 6 %; flags 2.5–3 %; the D3DX SSE path about 3 %; DXVK's early-out 2–3 %; FEX's L2 lookup cache at
most 0.6 %. Together about 1.3–1.5× at a given clock, so NFSU2 at about 40 fps would reach 50–55. No `dxvk.conf`
option or DXVK fork (Sarek, gplasync) speeds up the game thread: their gains come from drivers without graphics
pipeline libraries, and NVK on the console has them (`graphicsPipelineLibrary : 1` in DXVK's log).

The CPU clock is the one lever measured on the console: about 30 fps at 1020 MHz and 40–60 fps at 1785 MHz. Carafe
leaves the clock to the player (sys-clk, HOC) and does not set it.

The study's patches are in `runtime/patches`. The ones the console runs proved are built in: on in the runtime, with
a key Autorun still reads and no row in the settings. Single precision keeps its Experimental row and is on by itself
for games whose `.exe` imports `d3d8.dll` or `d3d9.dll` (its value "auto", resolved when Carafe builds the NSP):

| Patch | Option, default | What it does | Checked |
|---|---|---|---|
| FEX `0006` | `FEX_X87CONVERSIONROUNDING`, built in | FLDCW leaves FPCR alone; FIST, FISTP and FRNDINT round by the guest's FCW | console: `_ftol` 4 % → 0.2 % |
| FEX `0007`–`0009` | `FEX_X87FASTSTACK`, built in | the stack pass loads earlier blocks' registers on demand, writes back only what changed; `fnstsw` reads only TOP; C0–C3 reused within a block | FEX's x87 tests; console: matrix multiply loop 69 → 43 instructions |
| FEX `0010` | `FEX_X87SINGLEPRECISION` (Experimental row "x87 in single precision"), auto | x87 values as float32 | console: `fcvt` 7–10 % → 0.9 % of the game's instructions |
| FEX `0011` | `FEX_FLAGSWITHOUTFLAGM`, built in | droppable carry fix-up, flags folded into jumps, cheaper carry inserts, dead NZCV inserts and carry inversions removed | FEX's ASM tests, 18000 fuzzer runs; console runs |
| FEX `0012` | with `0006` | FIST and FISTP convert with one `fcvtns`/`fcvtms`/`fcvtps`/`fcvtzs` per rounding mode, from the float32 value under single precision; RC 0 is round-to-nearest whatever SSE left in FPCR | console: 4.5 minutes of racing |
| DXVK `0004`, Autorun `0060`, `use-dxvk.py` | `d3d9.unchangedStateFastPath`, built in | unchanged states return before the device lock; `0060` lets Autorun copy the key into `dxvk.conf`; `use-dxvk.py` gives the 32-bit DXVK its `dxvk-manifest.json` | console: not applied before the manifest |
| Autorun `0062` | always | a second Vulkan surface shares the screen while the first holds only a reservation; a game's probe device on its first start no longer locks the real one out of the screen | lab: NFSU2's first launch stayed on the loading screen before it |
| FEX `0013` | always | a restarted compile drops the first attempt's RIP entries | — |
| Autorun `0061` | always | the ASLR region is traced once; with verbose traces on, each of the thousands of early reservations wrote a line to the SD card and start-up took minutes | console: start hung with verbose traces before it |
| Autorun `0053`, `0055`, `0058` | with the profiler | 4096-bucket tables with a 12-bit hash start, busiest JIT blocks to `logs/jit-blocks.txt`, FEX time outside blocks split into dispatcher parts | — |
| Autorun `0057`, FEX `0014` | `FEX_GDBSYMBOLS` (exact code map, with the profiler) | every x86 instruction in the block address map; `0014` keeps FEX from calling its Windows GDB stub, which ended the process on the first block | console: hung at start before `0014` |
| Overlay | always | `[CLOCKS]` line with CPU, GPU and memory clocks and the GPU's busy share (`nvhost-ctrl-gpu`, sampled every 100 ms); `[FPS]` per report; `[OPTIONS]` with the FEX and DXVK keys in force | — |
| DXVK `0005`, Autorun `0063` | with the profiler | `[D3D9 calls]` every 600 presents: draws, render, sampler and texture-stage states and textures with how many left the value as it was, stream and index buffers, constant sets, transforms, buffer locks and state block applies, per frame | — |
| Overlay | with the profiler, third report | `[A57]` cycles of one link of a dependent chain: FP add, multiply, conversions, `fcvt`, GPR↔FP moves, `mrs`/`msr nzcv`, carry inversion, store-to-load forwarding by size, `msr fpcr`, and three ways to round to an integer | console |

Autorun `0056` and `0059` pass the new FEX options from a program's settings to FEX.

Tried and dropped after the console run of 2026-10-26:

- `FlagsDeadAtCalls` (flags dead at a direct call into code that overwrites them): flags cost 2–3 cycles on the A57,
  the patch saved under 1 % and was never tested.
- `SyscallReturnStack` (the WoW64 bridge leaves as a return): the bridge is 0.3 % of the main thread, the most it could
  win.
- `FEX_HOSTFEATURES=enable3dnow` (3DNow! for old D3DX): it worked in this game, no black screen, and D3DX's matrix
  multiply fell from 1.08 % to 0.65 % of translated time, about 0.5 % of the main thread; FEX hides 3DNow! because
  native d3dx9 DLLs of other games show a black screen with it, too high a risk for that gain.
- A launch-by-launch rotation of option sets: one launch per option set asks too much of the player.

### Measured on the console, October 2026

The `[A57]` microbenchmark at 1887 MHz, cycles for one link of a dependent chain:

| Operation | Cycles |
|---|---|
| integer add (calibration) | 1.0 |
| `fadd` / `fmul` double | 5.0 / 6.0 |
| `fcvt` double→single→double | 10.0 (5 each) |
| `fcvtzs` + `scvtf` (FP→integer→FP) | 23.8 |
| `frinti` | 5.0 |
| `fmov` GPR→FP→GPR | 8.1 |
| `mrs` + `msr nzcv` | 2.0 |
| carry inversion without FlagM (`mrs`, `eor`, `msr`) | 3.0 |
| `msr nzcv` alone, throughput | 0.6 |
| `cmp` + `csel` | 2.0 |
| store then load of the same address, 8/16/32/64-bit | 6.5–7.0 |
| dependent load from L1 | 4.0 |
| `msr fpcr`, a pair | 74.1 |
| rounding to an integer: plain / switching FPCR / `0006`'s select | 24.1 / 113.4 / 31.6 |

What it decides:

- Writing FPCR costs about 37 cycles. Rounding by the x87 control word without touching FPCR (`0006`) is 3.6× cheaper
  than switching FPCR around each conversion; this is why `_ftol` fell from 4 % to 0.2 % of the main thread.
- Flags are cheap on this core: patches that only remove flag instructions win little in time.
- A value kept in a register instead of the thread state saves about 7 cycles per access in a chain, which is what
  the x87 stack in registers buys; each `fcvt` on a chain costs 5, which single precision removes from float32 loads
  and stores (from 7–10 % of the game's ARM64 instructions to 0.9 %).
- Conversions between FP and integer registers are the dearest common operation (about 24 cycles round trip).

The run with every remaining option on (3.5 minutes of racing, the main thread 98 % busy): 33–48 fps, no errors.
Translated code was 2.19 ARM64 instructions per x86 one in the game's hot blocks, against about 2.5 before the
patches. The main thread's time: the game's code 75 %, about half of it D3DX (effects re-applying states), `d3d9.dll`
19 % (`PrepareDraw`, state blocks, render, sampler and texture-stage state setters, texture reference counts), FEX's
block entries 6.8 % (the first two instructions of a block, where a mispredicted branch into it is charged).

Where the main thread's time sits, by the instruction under each sample of the 256 hottest translated blocks
(`tools/classify.js` over the block dump; a sample lands on the oldest instruction not yet done, so it shows what the
core waits on): loads and stores 67 % (stack 20 %, other memory 48 %), block headers 9 %, the lookup of indirect
targets 5 %, indirect branches 3 %, x87 state in memory 3 %, everything else 13 %. The headers are the first
instructions after a branch into a block; their share is the cost of mispredicted entries, most of them the `switch`
D3DX runs per effect state (`jmp [table + type * 4]`: the table load, FEX's lookup and `br` are 35 % of the hottest
D3DX block). Forwarding a value stored in a block to a later load of the same address would remove only 0.5 % of the
instructions. The next console run's `[A57]` measures the latency of L1, L2, DRAM and a TLB miss and the cost of a
mispredicted indirect branch, to tell how much of the memory share is cache misses.

Direct3D 9 calls per frame in the lab, in the menus (`[D3D9 calls]`, 600 frames): NFSU2 makes 149 draws from memory
and sets 1464 render states (98 % unchanged), 779 sampler states (99 %), 537 texture-stage states (100 %) and 149
textures (93 %), applies 31 state blocks and 185 vertex shader constant sets; NFSU makes 14 draws and 157 draws from
memory with 306 render states (91 % unchanged). About ten unchanged render states per draw is D3DX re-applying each
pass; the race on the console will give the counts that matter.

Single precision is decided per game by its Direct3D version, not by the precision control at run time. DXVK sets
24-bit precision on the thread that makes a device unless the game passes `D3DCREATE_FPU_PRESERVE`, so float32 is
what a Direct3D 8 or 9 game runs at on Windows; such a game with `FPU_PRESERVE` turns the row off. Following the
precision control when a block is compiled was weighed and left: this game's D3DX sets 64-bit precision around each
of its own calls, so its code and every block first compiled before the device would stay in double, and a block
shared by both contexts would take the precision of whichever ran it first.

`0010` keeps the x87 registers as doubles in the thread state, so single precision still pays an `fcvt` at block edges
and two in `FIST`; float32 in the thread state would recover at most 1–2 % but touches FSAVE, FXSAVE, MMX and thread
contexts, and waits behind the larger items.

Autorun took no option of its DXVK table for 32-bit games: it reads the DXVK version from `dxvk-manifest.json`, which
only the 64-bit payload had, so the version was empty and every option was skipped, Carafe's DXVK settings for
Direct3D 9 games included. `use-dxvk.py` now writes the manifest for the 32-bit `d3d9.dll`.

The run with the DXVK early-out applied and `0012` in (4.5 minutes, racing, no errors):

- 35–54 fps in the race, median 47 (the run above: median 37, other tracks); 60 fps, the screen's rate, in menus.
- The main thread 96–99 % busy: the game's code 77 %, `d3d9.dll` 16–18 %, FEX's block entries 6.6 %;
  `d3dx9_43.dll` 0 %, the game's D3DX is linked into its `.exe`. `dxvk-cs` is 35–40 % busy, all threads 1.5 cores.
- The GPU is 46–73 % busy in the race: the CPU is still the limit.
- Per frame in the race: up to 1500 draws and 260 draws from memory, 17700 render states (96.5 % unchanged), 14700
  sampler states (98.6 %), 7600 texture-stage states (99.8 %), 2700 textures (82 %), 2200 vertex shader constant
  sets, 300 state block applies.

| Memory and branches, `[A57]` | Cycles |
|---|---|
| dependent load from L1 (16 KB) / L2 (512 KB) / DRAM (4 MB) | 4.0 / 34.0 / 377.3 |
| one line per 4 KB page over 16 MB (TLB misses) | 356.4 |
| indirect branch to one target / to one of four at random | 1.0 / 24.4 |

A DRAM miss costs as much as 377 adds and a TLB miss adds nothing over it; a mispredicted indirect branch costs about
23 cycles. Whether the 67 % of samples on loads and stores are DRAM misses or L1 and L2 hits decides what moving work
to other cores can win, and needs a trace of the main thread's accesses.

## What we know

- The console has one CPU: four Cortex-A57 cores, ARMv8.0, 1020 MHz at stock and 1785 MHz in boost mode. Every change
  can be tuned and measured for this one core.
- 3D games we run are CPU-bound. Need for Speed Underground, the reference case, races at about 30 fps at 1020 MHz and
  40–60 fps at 1785 MHz with VSync on; the rate rose almost with the clock. With VSync, 30 can hide any real rate
  between 30 and 59; the rate without VSync is not measured.
- One game thread keeps one core busy: about 98 % in NFSU's race, 75–85 % in OpenTTD, with about 400 system calls per
  frame in NFSU.
- In Eden that thread spends most of its time in translated code, not in FEX itself. Eden emulates the CPU, so this is
  a hint only.

## What runs translated

- DXVK's `d3d9.dll` is an i686 PE (`runtime/dxvk/build-dxvk.sh:53`), so DXVK runs under FEX: its Direct3D 9 front end
  on the game's thread and its `dxvk-cs` worker on its own core (`d3d9_device.h:1227-1268`).
- Every DLL a game loads from `syswow64` is i386 and translated too: ntdll, kernelbase, kernel32, msvcrt, user32,
  winevulkan, dsound, dinput8.
- Native ARM64: ntdll, wow64, wow64win, `libwow64fex.dll`, and the unix side linked into `wine-nx-runtime.elf`
  (Wine's unix code, winevulkan's thunks, Mesa).
- A Vulkan call leaves the JIT through FEX's unix-call bridge (`Source/Windows/WOW64/Module.cpp:452-494`). No kernel
  transition, but a register spill and a re-entry each time.
- ARM64EC is used only for 64-bit games (`horizon-wine/source/runtime.c:3934-3937`). FEX's host thunks exist only for
  Linux and are off.

So a change to FEX speeds up the game's code, DXVK and Wine at once, in every 32-bit game.

## Measuring first

- The runtime's profiler (`profile.txt`, `thread_profile.c`) finds translated code only through Box64
  (`wine_nx_box64_pc_to_x86`, line 540). Under FEX all JIT code lands in "pe" as raw addresses, so it cannot tell a
  game from `d3d9.dll` or Wine. It needs the guest address from FEX's `CpuStateFrame`, the way
  `RestoreRIPFromHostPC` does (`Core/Core.cpp:154-188`).
- The DXVK HUD forces `dxvk.enableDescriptorBuffer = False` (`launcher_settings.h:237-242`): a run with the HUD takes a
  different path from a run without it.
- Frame rate can be measured only on the console; Carafe Lab checks that a change still works.

## Ways to win time

### Block-to-block jumps

FEX translates code in blocks and looks up the next one on every indirect jump, call through a pointer, and return.
Autorun caps the per-thread L1 lookup cache at 64K entries and leaves L2 off (`horizon-wine/fex/cache_policy.h:22-26`,
`source/fex_options.h:60-62`).

- **A clock read on every L1 miss** (checked). `UpdateDynamicL1Stats` calls `std::chrono::system_clock::now()` on
  every L2/L3 hit (`Interface/Core/LookupCache.h:237-242`), which on Windows is `NtQuerySystemTime`
  (`Source/Windows/Common/WinAPI/IO.cpp:412-416`). A cycle counter read would do. `FEX_DYNAMICL1CACHE=0` avoids it
  without a patch, at up to 1 MB per thread.
- **Most L1 slots unused** (checked). The slot is `RIP & mask` (`Interface/Core/JIT/BranchOps.cpp:209`); MSVC aligns
  functions to 16 bytes, so call targets fall into 1 of 16 slots. Hashing the address (`RIP ^ (RIP >> 4)`) in all
  four places that index L1 costs one instruction and no memory.
- **The call-return stack drifts on every system call and unix call.** Wine's i386 stubs reach the bridge with
  `call`, and the exit is a plain jump that never pops FEX's entry (`OpcodeDispatcher.cpp:62-66`,
  `WOW64/Module.cpp:452-484`). Each later return misses and goes through L1.

### x86 flags on ARMv8.0

The A57 has no FlagM. x86 and ARM keep the carry of a subtraction in opposite forms, and FEX fixes the form before
every conditional jump. Without FlagM this is `mrs nzcv; eor; msr nzcv` (checked: `OpcodeDispatcher.h:1763-1781`), and
it is emitted as a store of all four flags (checked: `RedundantFlagCalculationElimination.cpp:267-275`), so the
dead-flag pass can drop it only when every flag is dead. `test; jz` and `add; jcc` pay it every time, in any game.

- Emit `CarryInvert` on ARMv8.0 too and lower it as `mrs; eor; msr`: the pass already drops it when carry is dead
  (checked: same file, lines 291-296).
- Produce carry in the needed form to begin with for `test` and 8/16-bit logical operations
  (`OpcodeDispatcher.h:1690-1712`).
- Flag inserts (shifts, `inc`/`dec`, `bt`) as per-bit `RmifNZCV` lowered to `mrs; bfi; msr`
  (`OpcodeDispatcher.h:1719-1747`).
- The cost of `mrs`/`msr nzcv` on the A57 needs a microbenchmark on the console.

### x87

Games built with MSVC before SSE2 became the default do their float math on x87. With `X87ReducedPrecision` the x87
registers are doubles (`Core/Frontend.cpp:74-78`).

- **The x87 stack goes to memory at every block end** (checked: the pass resets per block,
  `IR/Passes/x87StackOptimizationPass.cpp:741-745`; flush at 1227-1233). The first read of a slot not pushed in the
  block moves the rest of the block to the slow memory path (`:494`). Every branch starts a new block, so branchy float
  code spills all the time. Loading such a slot on demand would keep the fast path.
- **`fnstsw ax`** syncs the whole stack to memory in the middle of a block (`OpcodeDispatcher/X87.cpp:771`), though it
  needs only TOP.
- **`fldcw`** forces the slow path and writes FPCR (checked: `OpcodeDispatcher/X87F64.cpp:48-57`). The older `_ftol`
  helper does it twice per float-to-int cast; VC7.1's `_ftol2` does not.
- The tag word is updated at every block exit, though only `fxam`, `fsave`/`fxsave`/`fstenv` and MMX read it.
- A quick check per game: `FEX_X87REDUCEDPRECISION=0` against `1`. A large drop means x87 is hot.

### Stutter

- New blocks are compiled on the thread that reached them, the game's main thread included
  (`Dispatcher.cpp:319-338`). Every FEX `malloc`/`free` and every instruction cache flush is a unix call
  (`horizon.patch:1329-1357`, `horizon-wine/source/fex_unix.c:101-129`).
- FEX-2609 has a disk cache of translated code, off and not exposed in `fex_options.h` (`Config.json.in:147-202`).
  The Horizon port loads cached code but turns off page-trap self-modifying-code detection with it
  (`horizon.patch:1534`). It would remove the stutter of code reached for the first time; it is real work to make safe.

### DXVK calls

- Recording `vkCmd*` calls in the i386 winevulkan and replaying them natively at submit would cut most bridge
  crossings on `dxvk-cs`. Weeks of work; worth it only where `dxvk-cs` turns out to be the limit.
- A native `d3d9.dll` behind a WoW64 thunk: about 16 COM interfaces and 300 methods, a native DXVK for Horizon,
  locked memory below 4 GB. Months; the largest single gain for the Direct3D side, a game's own code stays translated.

## Quick checks without code

- `cpu=box64` on the same scene: Autorun's Box64 has a unix-call gate written for NFSU2 (`wow64_box64_unix.c:17-24`).
- `FEX_DYNAMICL1CACHE=0`; `FEX_MAXINST` 2500–5000.
- `FEX_X87REDUCEDPRECISION=0` as the x87 check above.
- The same scene with and without the DXVK HUD.

Enabling L2 as it stands is not worth it: its 4 MB covers 64 guest pages of 64 KB (`LookupCache.h:481`), less than a
game's hot code. `TUNE_CPU` and LTO change only FEX's own C++, not the code it emits
(`CMakeLists.txt:21,220,546-551`).

## Expectation

Not measured. In a CPU-bound Direct3D 9 game the main thread likely splits between the game's own code, a large share
for the Direct3D 9 front end of DXVK, and the rest for jumps, Wine and system calls. FEX changes speed up all three in
every game; DXVK changes speed up only the Direct3D side. For NFSU, a steady 60 fps at 1785 MHz looks reachable;
60 fps at 1020 MHz needs 1.5–2 times faster CPU work, and only several of the ways above together could give it.

Order: the profiler that understands FEX and an A57 microbenchmark, run together with the quick checks; then the
block-jump and flag patches; then x87 and the disk cache as the profile shows; DXVK only if the profile points there.
NFSU's race and OpenTTD serve as the reference scenes.

FEX does not accept AI-written code (`CLAUDE.md` in its tree), so these patches stay in `runtime/patches/fex`.
