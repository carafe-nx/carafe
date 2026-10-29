# W7. Translated code for the A57: instruction choice, block layout, PRFM, profile-guided translation

## Answer

After the x87 and flag patches, picking instructions by their A57 cost has little left to win: the dear instructions
(`fcvt`, `mrs`/`msr nzcv`, GPR↔FP `fmov`, `msr fpcr`) are under 2.5 % of the ARM64 the game's hot blocks execute and
under 1 % of the samples, so that line is worth at most about 1 %. The time FEX itself adds now sits at block edges:
of the samples in the race's hot translated blocks, 7.7 % land on block headers, 4.5 % on the lookup that follows an
indirect call, 1.9 % on jump-table dispatch, and 2.9 % on returns that missed FEX's call-return stack and fell into
the lookup. Ranked by expected gain on the NFSU2 race (shares of the main thread, which is the limit):

1. **Indirect calls through monomorphic inline caches** (a compare against the one target seen, then a direct `bl`):
   2–3 %.
2. **Returns that miss the call-return stack**: 1–2.5 % if the misses come from the stack drifting at the
   system-call and unix-call bridge, as the code suggests (measurement W7-M4 decides).
3. **Guest jump tables compiled as internal multi-way branches of the multiblock** (`jmp [table + reg*4]` with the
   table in read-only `.text`, as MSVC emits it): 1–2 %.
4. **Slimmer block entries** (no state store of the block header at every entry and return point, and the interrupt
   check only at block starts and back edges): 1–3 %.
5. **Code layout for the 48 KB L1I** (cold jump thunks, block tails and miss paths out of line, hot blocks packed in
   profile order, block starts on 16-byte boundaries): 0–8 %. The hot code of the race is larger than L1I (80 % of
   samples fall in at least 74 KB of sampled 64-byte lines, 200 KB of whole blocks), but the miss rate is not
   measured; with the A57's slow code fetch from L2 (about one instruction a cycle) the upper end is real only if
   W7-M1 shows a steep fall-off past 48 KB.
6. **Profile-guided retranslation of the hot set** (superblocks through calls, the x87 stack, flags and guest
   registers kept across what are now block edges), compiled in the background: 2–5 % on top of 1–4, overlapping
   them in part.
7. **A register holding a module's data base** instead of `mov`/`movk` pairs for every global address: 1–3 %, mostly
   through fetch bandwidth and code size.
8. **`PRFM` for delinquent loads**: 0–5 %, with risk. Loads take 67 % of the samples, but the top 50 memory sites hold
   only 18 % of all samples, a fifth of the load samples sit on loads whose address came from a load just before,
   and the A57
   has one load pipe that `PRFM` shares. It waits for T1 (how much of the load time is DRAM).

Together, without double counting, 8–15 % less main-thread time, so the race at a median of 47 fps would reach about
51–55 fps while the GPU (46–73 % busy) keeps headroom. Items 1–4 are local to FEX's Arm64 backend and frontend and
can be built and checked one at a time; 5 and 6 need the per-game profile of W12 and the background compiler of W10.

## Evidence

Sources: FEX at `395b132` with Autorun's `horizon.patch` (`636913ca`) and Carafe's `runtime/patches/fex` 0001–0014
applied, the tree `runtime/build/fex-src`; the ARM Cortex-A57 Software Optimization Guide (UAN 0015B,
`D:\debug_switch\carafe\sources\cortex-a57-optimization-guide.pdf`, cited by section); the console runs
`nfsu2-exp7` and `nfsu2-exp9` (October 2026, 1887 MHz).

### How the block dumps were read

- `nfsu2-exp9/logs/jit-blocks.txt` holds the 256 busiest blocks of the whole run, 68969 samples, written at
  `SPEED2_profiler.log:1336`. The run includes six reports of loading, where the main thread spins on
  `QueryPerformanceCounter` (the blocks at `0x43bdf0`, `0x43be20`, `0x57ead0`, `0x75c600`, the system-call blocks
  `0x3c0000`/`0x3c0002` and the ntdll/kernel32 path). A spin loop's speed does not matter, so these blocks are left
  out and only `SPEED2.EXE` (0x400000–0x932000) and `d3d9.dll` (base 0xFC180000) are kept: the "race set".
- The disassembly is `tools/blocks7` (run `nfsu2-exp7`, 256 blocks, made with `dis.sh`). Codegen did not change
  between exp7 and exp9: of 138 blocks present in both, 132 have the same host size. Docker was not running, so
  exp9's own blocks were not disassembled; numbers below are exp7's code with exp7's samples unless marked exp9.
- A sample lands on the oldest instruction not yet done (the sampler pauses the thread every 2 ms,
  `thread_profile.c`), so a share is "time the core waited there", not instruction count.

Race set, exp7, by sampled instruction (one-off scripts over `blocks7`, same rules as `tools/classify.js`, with block
headers told apart from call-site `adr`s):

| Where the samples sit | Share |
|---|---|
| loads | 66.9 % |
| stores | 12.7 % |
| block headers (`adr x0`, `str x0, [x28]`, `str xzr, [x28, #0xec0]`) | 7.7 % (5.5 % at block starts, 2.2 % at return points) |
| the three instructions after a header | 6.6 % |
| lookup sequence of indirect calls (`eor` … `blr`) | 4.5 % (491 sites) |
| lookup sequence of jump tables (`eor` … `br`) | 1.9 % (22 sites) |
| returns that missed the call-return stack (lookup … `ret`) | 1.9 % (272 sites), plus 1.0 % on the pop and compare before it |
| call-return pushes (`adr`, `stp … [x25, #-0x10]!`) | 0.6 % |

(Shares overlap: loads include the lookup's `ldp`.) `jit-stats.js` on exp9 gives the exe's hot blocks 2.30 ARM64 per
x86 instruction with `mrs nzcv` 0.57 %, `msr nzcv` 0.57 %, `fcvt` s↔d 0.95 %, GPR↔FP `fmov` 0.09 %, `msr fpcr` 0 %;
`classify.js` puts flags save/restore at 0.3 % and FP conversions at 0.1 % of samples.

### What FEX emits at block edges

- **Every entry point stores the block's header pointer into the thread state and touches the interrupt page**:
  `adr TMP1, header; str TMP1, [STATE, InlineJITBlockHeader]` (`JIT.cpp:795-799`), then `str xzr` to
  `InterruptFaultPage` (`JIT.cpp:769-782`), which WoW64 always turns on (`Core.cpp:371-374`). The header store exists
  so `RestoreRIPFromHostPC` can map a host PC to a guest RIP (`Core.cpp:154-190`) and so Carafe's profiler can name
  the block (Autorun patches 0050, 0058).
- **Every call's return point is an entry point**: the frontend adds the instruction after a call as a branch target
  and an entry point (`Frontend.cpp:1153-1164`), and the backend binds the return label before `EmitEntryPoint`
  (`JIT.cpp:964-981`), so each return pays the three header instructions and two stores. In the race set that is
  2191 return-point headers against 233 block starts.
- **Block starts sit at offset 4**: the block begins with a 4-byte `JITCodeHeader` and the code buffer hands out
  16-byte aligned slots (`JIT.cpp:908-917`, `JIT.cpp:1124-1129`); every block map in `jit-blocks.txt` begins
  `0x4:<rip>`. The guide asks for branch targets on quadword boundaries so the fetch after a taken branch gets four
  instructions (guide 4.8).
- **Indirect jumps and calls** load the L1 lookup table entry and branch (`BranchOps.cpp:204-239`): `eor` hash,
  `ldr` mask, `and`, `ldr` table pointer, `add`, `ldp` entry, `sub`, `cbz`, then `br`/`blr`; on a miss the same
  register is loaded with `DispatcherLoopTop`. The table is per thread, 16 bytes an entry, dynamic by default
  (`Config.json.in:286-292`): 8K entries (128 KB) growing to Autorun's cap of 64K (1 MB) (`LookupCache.h:479-486`,
  `horizon-wine/fex/cache_policy.h:22-24`). Patch 0005 hashes the slot (`LookupCache.h:378-380`), so hot targets
  are spread over the whole table, well beyond the 32 KB L1D and its 32-entry DTLB.
- **Returns** pop FEX's call-return stack (`ldp … [x25], #0x10`), compare and `ret x1` (`BranchOps.cpp:197-202`);
  calls push the guest return address with the host return label (`BranchOps.cpp:167-175`, `223-233`) and use
  `bl`/`blr`, so the A57's own return stack predicts the `ret`. When the pop does not match, the return goes through
  the L1 lookup. No call in the race set pushes a zero pair (0 of 2242 pushes), so the misses are mismatched returns,
  not unknown return blocks. The game-speed report found the stack drifting at every system call and unix call: the
  i386 stubs enter the bridge with `call` and leave it with a jump (`OpcodeDispatcher.cpp:62-66`,
  `WOW64/Module.cpp:452-484`). One stale entry makes every later return at outer levels miss until the stack is
  reset. The dropped `SyscallReturnStack` patch was judged by the bridge's own 0.3 %, not by these misses.
- **Direct branches** are linked by patching one `b`/`bl` (`JIT.cpp:531-615`), and the jump thunks of every exit
  (`b`, `br`, `ldr`, `blr` and three 8-byte words) stay inline after the block's code (`JIT.cpp:1020-1048`), followed
  by the block tail and the RIP entries (`JIT.cpp:1050-1106`). In the race set the blocks are 493 KB of code, 68 KB of
  thunks and 78 KB of tail and maps. The tail is larger in these runs because the profiler sets `FEX_GDBSYMBOLS`.
- **Multiblock** follows conditional and plain direct jumps forward of the entry, up to 16 KB ahead
  (`Frontend.cpp:1197-1201`), never into a call's target and never through an indirect jump.

### The hottest D3DX block

`0x669286` (D3DX's ApplyPass state loop, `exe+0x269286`) ends with the effect-state `switch`: guest `0x66985e`
loads the state record's type, `shr eax, 24`, `dec`, `cmp eax, 0x10`, `ja`, then `jmp [0x66a1e7 + eax*4]`. The table
is in `.text`, right after the function, as MSVC places it. In exp9 the type load has 162 samples, the guest table
load 153, the lookup's `ldp` 238 and the `br` 389 (host `+0xd8c` to `+0xddc`). The case blocks it jumps to start
with headers that are among the hottest in the run (`0x669877` 263, `0x669893` 98, `0x6698b4` 98, `0x669f30` 59 in
exp9). In the race set that is about 1180 of about 42000 samples, 2.8 %. Each case is a separate block reached
through the L1 table, although every target is a few hundred bytes from the jump and inside the multiblock's range.

### The A57

From the optimization guide:

- Three µops dispatched per cycle, at most one branch, two integer, one F0, one F1 and two load/store (4.1). One
  load and one store per cycle (4.5).
- `PRFM` runs in the load pipe with the same latency and throughput as a load (3.8).
- GPR→FP `fmov` and FP→GPR conversions also use the load pipe; FP→GPR conversion latency 10 (3.11). Store pair of X
  registers has throughput 1/2 (3.9), so FEX's `stp x, x0, [x25, #-0x10]!` call-return push costs two store-pipe
  cycles.
- `MOVZ`/`MOVK` and `MOVK`/`MOVK` pairs fuse into one cycle when adjacent (r1p0 and later, 4.14); `ADRP`+`ADD` fuses
  only from r1p3 (4.15). The Switch's A57 revision is said to be r1p1 (not verified).
- No more than two taken branches per 16-byte quadword, branch targets on quadword boundaries (4.8). `MSR` of NZCV
  is neither serialised nor in-order (4.12), which matches the measured 0.6-cycle throughput. FPCR writes are
  self-synchronising (4.16), which matches the measured 37 cycles each.

From the Cortex-A57 TRM (via its feature list): L1I 48 KB, 3-way, 64-byte lines; dynamic prediction with a BTB, a
global history buffer, a return stack and an indirect predictor. From Chips and Cheese's measurements of the Switch's
A57 (secondary source, [link](https://chipsandcheese.com/p/cortex-a57-nintendo-switchs-cpu)): a 64-entry micro-BTB
(taken branch without bubble) and a 2K–4K-entry BTB with a 2-cycle bubble; an indirect predictor of 512 entries
tracking up to 16 targets a branch; a return stack of about 32; code fetch from L2 of about one instruction a cycle,
"branch latency drastically increases" once code leaves L1I; ROB of 40 bundles; 32 loads and only 12 stores in
flight; 32-entry DTLB; an L2 prefetcher that issues up to eight prefetches after an L2 miss, all inside one 4 KB page.

From the `[A57]` microbenchmark on the console (`SPEED2_profiler.log`, game-speed report): L1 4 cycles, L2 34, DRAM
377, TLB miss 356 (a page walk that misses), indirect branch to one target 1, to one of four at random 24.4,
store-to-load forwarding 6.5–7, `fcvt` 5, GPR→FP→GPR 8.1, `mrs`+`msr nzcv` 2.

### Hot code against the instruction cache

Race set, exp9 weights, blocks sorted by samples:

| Share of samples | Blocks | Whole blocks | 64-byte lines that took samples |
|---|---|---|---|
| 50 % | 26 | 94 KB | 31 KB |
| 70 % | 57 | 162 KB | 57 KB |
| 80 % | 86 | 214 KB | 79 KB |
| 90 % | 134 | 360 KB | 119 KB |

exp7 gives the same picture (80 %: 81 blocks, 200 KB whole, 74 KB sampled lines). Sampled lines are a lower bound of
the code run: a line that executes without being sampled is still fetched. So one frame's hot code is several times
the 48 KB L1I. How often it misses is not known without the PMU (C6) or a microbenchmark.

### Memory samples

Top load and store sites in the race set (exp7): the 10 busiest hold 9.6 % of memory samples, 50 hold 22.5 %
(17.9 % of all samples), 200 hold 42.8 %. 13.7 % of all samples are on loads whose base register came from a load at
most three instructions before (pointer chasing). Among the busiest are `ldrb w20, [x20]` and
`ldr w5, [x10, #0x1c]` in `0x5d6570`, `ldrh w20, [x10, #0x18]` in `0x40cbc0`, and the L1 lookup `ldp x1, x0, [x0]`
in `0x669286` (361 samples): FEX's own table load is one of the six slowest loads of the race.

### What the others do

- Indirect branches in software translators (Hiser et al., TACO 2011,
  [pdf](https://people.cs.pitt.edu/~childers/papers/taco2011-indirect.pdf)): a large inline-looked-up translation
  cache (FEX's L1) is the best general mechanism, and inline tag compares against immediates win where a site has
  one or few targets.
- Tiaozhuan (TACO 2024, [doi](https://dl.acm.org/doi/10.1145/3703355)): a full guest-to-host address map plus a
  fault in place of the target check brings a lookup to one or two instructions: 3.9 % average and up to 19.4 % on
  SPEC CPU2006.
- MAMBO-X64 (PLDI 2017, [doi](https://dl.acm.org/doi/abs/10.1145/3062341.3062371)), a 32-bit-to-AArch64 translator on
  Cortex-A53 and X-Gene: under 7.5 % overhead, in part from traces built so the hardware return predictor works.
- HP Dynamo (PLDI 2000, [doi](https://dl.acm.org/doi/10.1145/349299.349303)): hot traces formed at run time (next
  executing tail) made `-O` binaries run like `-O4` ones, mostly from layout and removed indirect branches.
- Box64 (`docs/USAGE.md`, [link](https://github.com/ptitSeb/box64/blob/main/docs/USAGE.md)): `CALLRET` skips its jump
  table for returns, `BIGBLOCK` builds blocks as large as possible, and `DYNACACHE` keeps translated code on disk.

## The changes, one by one

Gains are shares of the main thread's time, assuming the hot blocks stand for the rest of the translated code
(about 96 % of the main thread in the race).

| # | Change | Where in FEX | How hard | Gain | Reasoning |
|---|---|---|---|---|---|
| a | Instruction choice by A57 cost | `JIT/*Ops.cpp` | small, per op | ≤ 1 % | the dear ops are < 2.5 % of executed ARM64 and < 1 % of samples; FP→GPR conversions use the load pipe, so they matter only in load-bound x87 code |
| b1 | Monomorphic inline cache at indirect calls | `BranchOps.cpp` ExitFunction, a link record like `ExitFunctionLinkData`, `JIT.cpp` ExitFunctionLink | medium | 2–3 % | 4.5 % of samples in the lookup; a literal compare and a patched direct `bl` replace the table load and the indirect predictor; COM vtable calls (D3DX → `d3d9.dll`) have one target a site |
| b2 | Call-return stack kept in step at the bridges | `OpcodeDispatcher.cpp:62-66`, `WOW64/Module.cpp:452-494` (the dropped `SyscallReturnStack`) | small | 1–2.5 % if W7-M4 confirms drift | 1.9 % + 1.0 % of samples on missed returns |
| b3 | Jump tables in `.text` compiled as internal multi-way branches | `Frontend.cpp` (decode the targets as multiblock targets), new IR op, host table of offsets in the block | medium | 1–2 % | 1.9 % in the lookup plus about 1.2 % on the case blocks' headers; the cases become internal labels without headers; the mispredict itself stays |
| b4 | Header store and interrupt store only where needed | `JIT.cpp:795-817`, `Core.cpp:154-190`, Autorun's profiler | medium: needs a host-PC → block map for faults and the profiler | 1–3 % | 7.7 % of samples on headers, but most of it is the cost of the redirect that lands there and moves to the next instruction; what is saved is three instructions and two of the 12 store-queue slots per entry, 2191 return points in the race set |
| c | Layout: thunks, tails and miss paths out of line, starts on 16 bytes, hot blocks packed in profile order | `JIT.cpp:1020-1106`, `SharedCodeBufferManager`, a hot region filled by W10's compiler | medium–large | 0–8 % | hot set ≥ 74–79 KB of sampled lines against 48 KB; 23 % of host bytes are thunks and tails; the upper end needs W7-M1 |
| d | `PRFM` for delinquent loads | IR pass with a per-site list from the profile | large, risky | 0–5 % | see below |
| e | Profile-guided retranslation (superblocks through calls, state kept in registers across edges, store-to-load forwarding inside the superblock) | new tier in `Core.cpp` CompileBlock, frontend following calls, W12's profile | large | 2–5 % beyond b–c | removes return-point entries, call pushes and pops, x87 state in memory (3.1 % of samples) and stack round trips through the 7-cycle forwarding path |
| f | Base register for a module's data | `OpcodeDispatcher` address generation, RA reserves one GPR | medium | 1–3 % | 9–13 % of ARM64 are `mov`/`movk` address pairs (game-speed report); they fuse, so the gain is fetch slots and I-cache bytes |

### PRFM in detail

- How a translator finds delinquent loads without precise PMU sampling: the 2 ms sampler already names the
  instruction a thread waits on; a site with many samples per execution of its block is delinquent. Execution counts
  can come from a counting build in the lab (Eden runs the same blocks, slowly but in the same order).
- What it would prefetch: a strided address (array walks; the A57's own prefetchers may already catch these within a
  page) or the next node of a pointer chain (needs the chain's next pointer early, which the guest code computes late).
  The address patterns of the hot sites can be traced in the lab, where only order matters, not time (shared with
  W5's trace of the main thread's accesses).
- Risks: `PRFM` takes the one load pipe in load-bound code (guide 3.8, 4.5); the L1D is only 2-way, so early
  prefetches evict live lines; wasted DRAM bandwidth is taken from the GPU, which shares the LPDDR4; a prefetch of a
  wrong address costs a TLB walk (356 cycles measured) with no result.
- Range: if the 50 busiest sites (18 % of samples) are L2 hits, the 40-bundle window already hides part of 34 cycles
  and `PRFM` wins under 1 %; if they are DRAM misses with predictable addresses, prefetching two or more iterations
  ahead could recover up to half of them, about 5–9 %, less the pipe cost. T1 decides which.

## Measured in the lab (2026-10-29)

FEX's own count of returns that miss the call-return stack (FEX `0015`) does not depend on the host, so the lab gives
it as exactly as the console. In NFSU2's title menu, main thread, 7–13 million of 25–33 million returns per report
miss: 25–50 %.

A first round of A/B runs changed nothing, for a reason outside FEX: the runtime passes FEX only the options in
Autorun's own list (`horizon-wine/source/fex_options.h`), so a new FEX option written into the program file never
reached FEX until Autorun `0073` added it. Those runs compared the same code twice.

A recorder in a diagnostic build, behind the profiler's statistics, counted the pairs of the address popped and the
address returned to:

| Popped | Returned to | Count in 2 million misses |
|---|---|---|
| `ntdll.dll` after a call | `ntdll.dll` | 381 589 |
| `ntdll.dll` after a call | the game's code (`0x6f5eab`) | 193 267 |
| the game's code (`0x6f5eab`) | the game's code (`0x6f5ed2`) | 193 257 |
| `winevulkan.dll` (`vkCmdDraw` + 0x31) | `d3d9.dll` | 88 304 |

Each return pops the address the previous return wanted: the stack holds one entry too many. The extra entries are
return addresses in `ntdll.dll` and `winevulkan.dll` right after their calls into the system-call and unix-call bridge
(`__wine_unix_call` from `vkCmdDraw` and the like). The bridge is entered by a call and goes on at the return address
without a return, so the call's entry stays on the stack, and every later return of the outer frames misses until the
stack fills up and is reset. b2 (FEX `0017`, the bridge leaves as a return) removes exactly that entry; with Autorun
`0073` it reaches FEX.

b1 (the indirect call cache) is FEX `0016` (decision 44); it reaches FEX through Autorun `0073` as well.

b3 (jump tables) is FEX `0018` (decision 47), passed on by Autorun `0074`. W7-M2 had already run in Carbon's start-up
(`[A57] jump tables`, 1785 MHz): an indirect branch 27.7 cycles a step alternating between two targets, 13.5 cycling
through four, 24.9 at random among four, a compare chain 18.8 at random among four. So up to four distinct cases are
compared one by one, and larger switches use a host table.

W7-M1 and W7-M3 now run with the other `[A57]` benches at start-up under the profiler (`[A57] code at …`): cycles an
instruction over 16–128 KB of straight code with a taken branch every 32 bytes, and cycles a block entered by `br`
with 8 or 16 stores, with and without the two header stores.

Measured on the console (Carbon, 1785 MHz, 2026-10-29, `nfsc-exp8`):

| Code size | 16 KB | 32 KB | 48 KB | 64 KB | 96 KB | 128 KB |
|---|---|---|---|---|---|---|
| Cycles an instruction | 0.51 | 0.50 | 0.65 | 1.64 | 2.14 | 2.11 |

| Block entered by `br` | 8 stores | with the header stores | 16 stores | with the header stores |
|---|---|---|---|---|
| Cycles a block | 8.0 | 10.0 | 16.2 | 18.2 |

- W7-M1: past the 48 KB L1I an instruction costs 3.2 times as much at 64 KB and 4.2 times at 96–128 KB, far over the
  1.3× of the rule above, so layout (c) moves up to the first three. The hot set of the race (74–79 KB of sampled
  lines) sits on the steep side: every byte of thunks, tails and miss paths taken out of the hot lines counts.
- W7-M3: the two header stores cost 2 cycles an entry, as stores run one a cycle; b4 saves those and 12 bytes of
  code an entry, which also counts against W7-M1.
- The same run had `FEX_JUMPTABLES` on: the host-table sequence shows in three of the 256 busiest blocks (guest
  `0xfce7c5e0`, `0xfce7c720`, `0xf66d5180`), and the race ran without faults. Its speed is not measured: the car stood
  at the start, held at 60 frames a second.

## What this means for the orchestrator

- The orchestrator does not need to pick instructions at run time; those choices are fixed in FEX's backend, once.
- It does own the choices that need knowledge of the run: which call sites are monomorphic, which blocks are hot,
  in which order to lay them out, which loads to prefetch. These come from W12's per-game profile and are applied by
  W10's background compiler, on a free core, into a hot code region, never on the game's thread.
- Items b1–b4 change no decisions at run time and can ship first, each behind its own Experimental row, off by
  default; c and e follow when W10 and W12 exist.
- Every item changes emitted code, so each changes the disk cache's hash and the cache is rebuilt (translated-code
  cache report).

## Needs the console or the lab

| # | Measurement | How | Decision it switches |
|---|---|---|---|
| W7-M1 | Instruction fetch against code size | `[A57]`: a loop over 8, 16, 32, 48, 64, 96, 128, 256, 512 KB of code (integer ops with a taken branch every 16–32 bytes), cycles per instruction at each size | If cycles per instruction past 48 KB rise more than 1.3×, layout (c) moves up to the first three; if under 1.1×, it drops to the last |
| W7-M2 | Indirect branch prediction against pattern | `[A57]`: one `br` with 4, 8, 16 targets in a repeating sequence of period 4–64, against a compare chain of the same targets | Whether jump tables become a host jump table (b3) or a profile-ordered compare chain |
| W7-M3 | Cost of block-entry stores | `[A57]`: a loop of blocks entered by `br`, with and without the two header stores, and with 8 or 16 other stores in the block | The rank of b4 |
| W7-M4 | Returns that miss the call-return stack | profiler: share of samples in the miss path (lookup sequences ending in `ret`) per report, with `SyscallReturnStack` on and off | Whether b2 is built; if the share does not drop, the misses have another source and b2 is dropped |
| W7-M5 | L1 lookup table size in use | profiler: read `State.L1Mask` of each sampled thread and log it with `[PROF]` | Interpreting the lookup's load cost; whether a fixed smaller table or b1 is the cheaper fix |
| W7-M6 | PMU: L1I refills, branch mispredicts, L1D refills | if C6 finds the PMU readable: per-thread counts in `[PROF]` | Splits the 7.7 % header share between I-cache misses and mispredicts; ranks c against b; replaces W7-M1 and W7-M3 |
| W7-M7 | Correctness of each codegen change | FEX's ASM tests and the differential fuzzer on the PC; Carafe Lab: the same frames with the option on and off | Each item ships only after it |
| W7-M8 | Frame time with each item | console: one Experimental row per item, the race route, `[FPS]` and main-thread busy share | The order of the stages |

Not verified: the A57 revision (r1p1), the microarchitecture sizes from Chips and Cheese, and that FEX's lookup-table
loads miss L1D (inferred from their samples).
