# T1. What the main thread's loads and stores wait on

## Answer

Most of the main thread's time on memory instructions is short-latency work, not DRAM. Of the main thread's time in
the race (NFSU2, 1887 MHz, 2026-10-27 run), about 66 % sits on loads and stores and about 28 % on the first
instruction after a branch, which is front-end time (mispredicted branches and instruction fetch) that the earlier
"67 % on loads and stores" partly counted as memory. Split by the average cost of each sampled load or store per
execution: **DRAM-class sites (more than 60 cycles) take 3–7 % of the main thread, L2-class sites (8–60 cycles)
5–15 %, and L1-class latency chains (L1 hits at 4 cycles, store-to-load forwarding at 6.5–7) 45–55 %.** The DRAM
share is concentrated: two dependent loads in one function, a visibility bit-table test reached from a recursive
scene walk, take about 2.6 % of the main thread at 80–550 cycles a call; first touches of Direct3D objects in DXVK
and atomic reference counts (100–135 cycles each) add about 1 %. The L1-class bulk is translation-shaped: x86 stack
traffic (about 16 % of the main thread), FEX's own state in memory (x87 registers and TOP, the block lookup table, the
return stack: about 11 %), and pointer chains through game and D3DX objects that hit L1. The method is
calibration of sample counts against exact call counts from DXVK's `[D3D9 calls]`; its error is a factor of up to 5
in cycles per execution for functions without a known call count, and it cannot tell a load that always hits L2 from
one that sometimes misses to DRAM. A block-entry counter in the profiler or a memory trace in the lab settles it.

## Evidence

### Data

- `D:\debug_switch\carafe\nfsu2-exp9\logs\jit-blocks.txt`: 256 busiest FEX blocks, 68969 samples, written at
  `autorun_runtime.log` line 1379 (`SPEED2_profiler.log` line 1336, the same lines 43 earlier). The samples are pooled
  over the four sampled threads and over the whole run up to t ≈ 166 s, menus included (patch
  `runtime/patches/autorun/0055`: `nx_prof_add( &jit_blocks, block )` for every sampled thread).
- Replication: `nfsu2-exp7\logs\jit-blocks.txt` (65254 samples, `autorun_runtime.log` line 1327). Every figure below
  that has an exp7 counterpart agrees within 2 points.
- Main thread in steady race windows (`[PROF] 4w` with x86 > 90 %, 16 windows, `autorun_runtime.log` lines
  1031–1846): x86 96.3 %, pe 0.7 %, native 1.1 %, svc 1.7 %; exe 77.3 %, `d3d9.dll` 17.5 %.
- `[A57]` at 1887 MHz (`autorun_runtime.log` lines 836–838): L1 4.0, L2 34.0, DRAM 377.3, TLB-miss walk 356.4 cycles;
  store then load of the same address 6.1–7.0; indirect branch to one of four targets 24.4.
- Direct3D 9 calls per frame (`[D3D9 calls]`, lines 798–1382, 13 lines × 600 frames up to the block dump): render
  states 56.2 M, sampler states 45.5 M, texture-stage states 24.0 M, textures 8.3 M, draws 4.1 M, draws from memory
  1.45 M, vertex shader constant sets 7.5 M, stream sources 2.1 M, index buffers 2.1 M, state block applies 1.2 M.

### Scripts (all in `D:\debug_switch\carafe\tools\orchestrator\`)

- `blocks9\`: hex of each block (`..\dump-hex.js`), disassembled with `..\dis-all.sh` in `carafe-runtime:latest`.
- `mem.js`: classifies each sampled load and store by the address it uses, following register provenance inside the
  block: `x28` (FEX thread state), `x28 + TOP×16` (x87 register slots), `x25` (FEX return stack), the lookup-table
  chain (`ldr [x28,#0xa0/#0xa8]` → `ldp x1, x0, [x0]`), `x8` and `x9` copied from it (guest stack), constants built
  with `mov`/`movk` (guest globals), registers loaded in the block (pointer chains), registers from earlier blocks. It
  also marks a load that reads an address stored earlier on the same straight-line path (store-to-load forwarding).
- `runs.js`: splits blocks into straight-line runs and separates the first instruction of each run (after a branch or
  at a branch target) by the kind of branch before it.
- `yard.js`: cycles per execution of each sampled load, exact where the call count is known, otherwise against a
  yardstick (below). `percall.js`: cycles per call and per instruction for counted functions. `heavy.js`: load
  samples against neighbouring loads.
- Filters: game blocks (`0x400000–0x932000`) and DXVK functions that run on the main thread (`D3D9DeviceEx::*`,
  `D3D9StateBlock`, constant buffers, reference counts; symbols from the i386 `d3d9.dll` with `..\pesyms.js`, list in
  `blocks9\d3d9-syms.txt`). Excluded: the menu wait loops (`0x43bd00–0x43bf00`, `0x57ea00–0x57ef00`, `0x75c600`), the
  game's audio code `0x720000–0x740000` (its buckets are the audio thread's in `[PROF] 104w x86`), and DXVK's
  `dxvk-cs` functions. What remains is 32615 samples. Main-thread translated samples up to the dump were about 45000
  (exe 37634, of which about 5400 in menu wait loops; `d3d9.dll` 7328), so the 256 blocks cover about 82 % of the
  main thread's translated time.

### Where a sample lands

The profiler pauses the thread through `svcSetThreadActivity` from another core and reads its PC. On an out-of-order
core the interrupt is taken at the oldest instruction not yet completed, so a sample shows what the core was waiting
for. This is not stated in a document read for this note (not verified); the data agree with it:

- Interior ALU instructions get 0.02 samples a site (21918 sites), loads hundreds.
- An ALU instruction that consumes the load just before it gets 0.00 samples a site (3485 sites): no skid onto the
  consumer.
- In `SetRenderState`, called 56 M times, the busiest instruction costs 1.8 cycles a call and no instruction more
  (below): samples follow time, at fine grain.

### Front-end time, not memory

`runs.js`, main-thread blocks, exp9:

| first instruction of a run | sites | share of samples | mean samples a site |
|---|---|---|---|
| block entry (`adr x0`) | 162 | 2.8 % | 5.7 |
| fall-through after a conditional branch, ALU / load | 2154 / 1707 | 5.0 % / 10.6 % | 0.75 / 2.03 |
| target of a conditional branch, ALU / load | 2176 / 1238 | 7.4 % / 9.1 % | 1.11 / 2.41 |
| target of `b` and of several kinds | 732 | 3.7 % | 1.1–3.1 |

An ALU instruction that starts a run gets 35–55 times the samples of the same instruction inside a run: the time
there is refetch after a mispredicted branch or an instruction-cache miss at the branch target. The sampled 64-byte
lines of the 256 blocks make 194 KB of host code (58 KB for the busiest 64 blocks), against a 48 KB L1 instruction
cache, so both happen. Taking the ALU mean as the refetch cost of every run start, the front-end share is: block
entries 2.8 %, ALU run starts 13.9 %, the refetch part of loads that start a run 9.3 % (of their 22.5 %), and block
header stores right after an entry 3.0 %: **29 % of translated time (26–32 % with ±30 % on the refetch estimate),
28 % of the main thread.** What is left on memory instructions is 68.5 % of translated time.

### What the memory instructions touch

`mem.js`, main-thread blocks, exp9 (share of all samples in them; loads that start a run included):

| address | loads | stores | note |
|---|---|---|---|
| guest pointer loaded earlier in the block (object chains) | 34.4 % | 1.4 % | 10.3 points start a run |
| guest stack (`x8`, `x9 = x8`) | 9.1 % + 2.5 % after a store to the same slot | 5.2 % | spills, arguments, locals |
| guest address computed from registers of earlier code | 5.2 % + 1.0 % forwarded | 0.7 % | |
| guest pointer held from an earlier block | 4.3 % | 0.1 % | |
| guest globals and constant-indexed tables | 2.3 % | 0.0 % | |
| FEX lookup table of block addresses | 4.3 % | 0.0 % | three loads per indirect branch |
| FEX x87 register slots and TOP/tag bytes | 2.1 % + 1.2 % + 1.1 % forwarded | 1.4 % | x87 stack kept in memory |
| FEX return stack and thread state | 0.9 % | 0.6 % | |
| FEX block header (`str x0, [x28]`, `str xzr, [x28, #0xec0]`) | | 3.0 % | entry cost, counted as front-end above |

Store-to-load forwarding is visible directly on 4.8 % (loads of a slot stored earlier on the same path); it costs
6.5–7 cycles a link (`[A57]`). The exp7 table matches within 1.5 points on every row.

### Cycles per execution where the call count is known

`percall.js`, cycles per call = samples × 3.81 M cycles per sample (10 s per about 4950 samples at 1887 MHz) / calls:

| function (block) | calls | cycles a call | busiest loads, cycles a call |
|---|---|---|---|
| `SetRenderState` (`0xfc195d00`) | 56.2 M | 21.8 | `ldr [x20,#0x928]` 1.6, `ldr [x20],#4` 1.4, `ldr [x8,#8]` 1.2 |
| `SetSamplerState` (`0xfc197870`) | 45.5 M | 28.6 | 2.6, 2.6 |
| `SetTextureStageState` (`0xfc1975c0`) | 24.0 M | 44.2 | `ldr [x20,#0x13a8]` 7.0, 4.8 |
| `SetVertexShaderConstantF` | 7.5 M | 154 | source data `ldr q16/q17` 10–13 |
| `DrawIndexedPrimitive` | 4.1 M | 188 | 22.5, 12.2 |
| `PrepareDraw` (per draw, both kinds) | 5.5 M | 436 | eight loads of 8–21 |
| `SetStreamSource` | 2.1 M | 405 | vertex buffer object `ldr [x5,#0x3c]` **136**; `ldaxr` **102** |
| `SetIndices` | 2.1 M | 346 | `ldaxr` and the instruction after the exclusive pair **118 + 135** |
| texture `AddRef` / `Release` | ≤ 8.3 M | ≥ 136 / ≥ 135 | the instruction after `stlxr` **80** |
| D3DX `ApplyState` (`0x669286`) | ≤ 134 M (all states) | ≥ 66 | record `ldrh [x4,#0x54]` 9.4, lookup `ldp` 6.8, `br x1` 11.1 |

Over these exactly counted blocks (4853 samples, 15 % of the main thread's hot samples), loads take 57 % of the
samples: L1 class 40 %, L2 class 12.5 %, DRAM class 4 %. The DRAM-class part is not DRAM latency only: the
exclusive load/store pairs of reference counts cost 100–135 cycles. The same objects' counts are changed by `dxvk-cs`
on core 3 (its hot blocks spend 9.8 % on exclusive pairs, the main thread's 2.4 %), so the line probably moves between
the cores each time; this is not verified.

`ApplyState`'s first touch of a 0x64-byte state record (`ldrh w20, [x4, #0x54]`, guest `0x6692bc`) averages 9.4
cycles, or up to about 13 if D3DX makes 70 % of the state calls: at most 2.5 % DRAM misses or 30 % L2 hits on it.

### The yardstick for the other functions

For a block without a call count, the loads of its entry run (executed once per entry) are taken to cost 0.5–2.5
cycles each; this range is what the entry runs of the ten counted functions give (0.56–2.55 cycles). A site's cycles
per execution are then its samples over the entry-run mean, times 0.5–2.5. Loads inside loops are not rated (the
loop body of a multiblock is too uneven for a median). `yard.js`, main-thread blocks:

| class (average per execution) | rated, not in loops (33 % of samples) |
|---|---|
| L1, up to 8 cycles | 20–28 % |
| L2, 8–60 cycles | 1.8–9.4 % |
| DRAM, over 60 cycles | 3.2–3.7 % |
| in loops, not rated | 25.5 % |
| block without an entry-run yardstick | 10.3 % |

The DRAM-class sites are mostly two loads in one function, exe `0x5d6570`:

```
  355  ldr  w5, [x10, #0x1c]   ; guest 0x5d6596: mov edx, [esi+0x1c]    row of the table (esi = arg*64 + [eax+0x20])
    5  ldr  w11, [x4, #0x30]   ; guest 0x5d65a6: mov edi, [eax+0x30]    table base
  488  ldrb w20, [x20]         ; guest 0x5d65b0: test bl, [edx+edi]     bit (row*128 + col/8)
```

The other loads of the function get 1–3 samples each. The yardstick gives 80–390 and 110–550 cycles per execution.
The function is a bit-matrix test (rows of 128 bytes, a 64-byte record per node), called from `0x5d69b0`, which a
recursive function `0x5e99c0` reaches through a pointer; it is a visibility or region-overlap test of the scene walk
(the meaning is inferred). It is 2.6 % of the main thread's hot samples (exp7: 3.5 %) and appears as the frame pair
`exe+0x1d6a74<exe+0x1e9a18`, 2.2 % (exp9) and 3.0 % (exp7) of the main thread.

### The estimate and its error

Main thread = 100 % (translated code 96.3 %; Wine, Horizon and system calls 3.5 %):

| time | share of the main thread | how |
|---|---|---|
| front-end at run starts (mispredicts, instruction fetch, block entries) | 25–31 % | `runs.js`, ALU mean as refetch cost |
| loads and stores | 63–69 % | the rest of the memory instructions |
| – DRAM class | 3–7 % | rated sites 3.2–3.7 %; loops at most the rated proportion (0–3 %) |
| – L2 class | 5–15 % | rated 1.8–9.4 %; counted functions 12.5 %; loops 0–7 % |
| – L1 class: L1 hits, forwarding, FEX state, x87, stack | 45–55 % | the remainder |
| ALU and other interior instructions | 2–3 % | |

Sources of error:

- Cycles per execution are averages. A site at 30 cycles is either always an L2 hit or an L1 hit with 7 % DRAM misses;
  the classes say which averages, not which mechanism.
- The yardstick spans a factor of 5. A site moves between classes only near a boundary; the DRAM-class sites sit
  at 80–550 cycles and stay there at either end.
- Loops (25.5 %) and blocks without a yardstick (10.3 %) are bounded by assumption, not measured. They are mostly D3DX
  (preshader interpreter `0x688e79`, input copies `0x662ecc`, evaluation `0x663a32`), x87 matrix code (`0x67192e`,
  loads from FEX's x87 slots) and draw lists (`0x5cf420`, `0x5c5930`).
- Menu frames are in the dump: about 2000 of 32615 samples are menu rendering. Per-thread pooling is removed by
  address only; a game function that also runs on the audio thread would be counted as main (not seen).
- A TLB miss (356 cycles) cannot be told from a DRAM miss (377).
- Overlapped misses count once; this is the right view for time, not for miss counts.
- The 18 % of translated time outside the 256 blocks is assumed to behave like the rest.

Bandwidth: 5 % of 41 M cycles a frame at 377 cycles a miss is about 5000 misses, 0.3 MB a frame, about 15 MB/s.
The main thread does not compete with the GPU for DRAM bandwidth in any way that matters (inferred, C4 not checked).

## What this means for the orchestrator

- A run-ahead or helper thread (W5) can hide at most the DRAM class and part of the L2 class: 3–7 % plus, at best,
  a fraction of 5–15 %. The DRAM time is in a handful of sites, so a targeted `PRFM` in the translated code of those
  sites (W7, profile-guided) reaches most of it without a thread; a helper is not justified by this profile.
- The bulk, about half of the main thread, is L1-latency chains that translation shapes: x86 stack slots through
  memory, x87 registers and TOP in FEX's state, three loads per indirect-branch lookup, block headers. Keeping stack
  slots and x87 registers in host registers across blocks, and cheaper lookups, act on it directly (W7). Moving whole
  work items to other cores (W1, W3, W4) removes it with the work.
- The front-end share (about 28 %) is as large as all non-L1 memory stalls together. Code size and layout of hot
  blocks (194 KB of sampled lines against a 48 KB instruction cache), hot/cold splitting inside multiblocks and fewer
  data-dependent branches are a real lever (W7).
- Atomic reference counts cost 100–135 cycles each and are shared with `dxvk-cs` on core 3. Moving more Direct3D 9
  work to another core (W3) multiplies such transfers unless reference counts stay with one core.
- The main thread barely uses DRAM bandwidth: moving its work to other cores will not starve the GPU.

## Needs the console or the lab

1. **Block entry counts (profiler, console).** FEX counts entries of every block while the sampler runs (a counter
   in the block header, an Experimental profiler option), and `jit-blocks.txt` writes the count next to the samples.
   Cycles per execution become exact for every site not in a loop, and loop sites get iterations from the counts of
   their back-edge runs. It switches: if DRAM plus L2 classes exceed about 15 % of the main thread, W5 and automatic
   `PRFM` move up; below, they stay behind translation work (W7) and moving work off the thread.
2. **Per-thread, race-only block dump (profiler, console).** Reset `jit-blocks` at the first race window and keep a
   dump per sampled thread. It removes the menu and the address filters from the error list. It switches nothing by
   itself; it makes 1 and 3 clean.
3. **Main-thread memory trace (lab).** Eden with fastmem off, dynarmic's memory callbacks logging each load and
   store of the core running the main thread, filtered by the FEX code range, for about 10 frames; replayed through a
   model of the A57 (32 KB 2-way L1D, 2 MB 16-way shared L2, 64-byte lines, a 32-entry L1 data TLB and a 1024-entry
   L2 TLB, 4 KB pages; the TLB sizes are from ARM's A57 reference manual as remembered, not verified here). Gives a miss rate per site and per address range, which the averages above cannot. It switches the same
   decision as 1, and names the addresses and strides a prefetch would need. Not verified: whether Eden reaches the
   race in reasonable time; the menu is the fallback scene.
4. **Cache and branch counters (console, if C6 allows).** `L1D_CACHE_REFILL`, `L2D_CACHE_REFILL`,
   `L1I_CACHE_REFILL`, `BR_MIS_PRED`, `INST_RETIRED`, `CPU_CYCLES` on the main thread per window. Settles the 28 %
   front-end estimate and gives misses per frame. It switches the order between code layout (front-end) and data
   prefetch work.
5. **Atomic between cores (`[A57]`, console).** An exclusive increment on a line last written by another core, core 1
   against core 3, against the same core. It switches whether W3 must keep reference counts thread-local.
6. **Prefetch probe (console, one Experimental build).** `PRFM` for the row and bit loads of `0x5d6570` (guest
   `0x5d6596`, `0x5d65b0`) and the record chain of `ApplyState`; fps and samples of those sites. It switches whether
   profile-guided prefetch is worth automating.
