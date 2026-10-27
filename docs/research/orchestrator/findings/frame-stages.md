# T3. The game's frame as stages, and which depend on which

## Answer

NFSU2 builds each frame on its main thread in one loop, `exe 0x5811c0`, in a fixed order: timers, messages and input;
the simulation (`0x609ce0`, the race world and its cars); a group of updates after it (`0x60aee0`, `0x5663b0`,
`0x600900`, `0x44efe0` and others); and the render (`0x5d2850`), which calls `BeginScene`, walks the scene and its
views (`0x5caad0`), draws every mesh through the D3DX effect framework linked into the `.exe` (Pass → ApplyPass →
ApplyState → Evaluate), calls `EndScene` and `Present`. This order is observed in the code; the names are inferred.
Measured shares of the main thread in the race: **Direct3D 9 (DXVK's front end on this thread) 17.5 %; D3DX effect
application 30–35 % (at least 27 % from frame pairs); the game's own render code (scene walk, visibility tests, draw
lists, per-object setup) 12–25 %; D3DX x87 math and C runtime helpers 6–7 %; the simulation only 3–15 % (3 % in code
only it reaches); Wine and system calls 3.5 %**; about 13 % of the thread sits outside the 256 dumped blocks and is not
attributed. The render path is two thirds to four fifths of the frame. Data flows one way through it: simulation →
updates → render → D3DX → Direct3D 9 → `dxvk-cs`; D3DX and Direct3D 9 return almost nothing to the game except at a
few synchronous points (dynamic buffer locks, draws from memory, `Present`). So the D3DX and Direct3D 9 work, about
half of the thread, is a stream of commands that could be produced on another core if the inputs are copied at the
call, and the render of frame N could overlap the simulation of frame N+1 if the state the render reads is
snapshotted; the simulation alone is too small to be worth splitting.

## Evidence

### What the profiler gives and what it does not

- `[PROF] 4w x86 frames` keeps, per sample, the two innermost return addresses of the x86 EBP chain, and the report
  lists only the 12 busiest pairs. Summed over the 16 steady race windows of exp9 (`autorun_runtime.log` lines
  1031–1846), the listed pairs cover 33.8 % of the main thread's samples. Functions without an EBP frame (DXVK's
  i386 code, many game leaf functions) are counted under the frame of their caller.
- `[PROF] 4w x86` lists the 12 busiest 32-byte buckets per window; in the race the busiest is 3 % (`exe+0x269860`):
  the thread's time is spread, so buckets alone do not give stages.
- `jit-blocks.txt` gives all samples of the 256 busiest blocks (83 % of the main thread's game-code samples up to the
  dump, after the filters of T1), but pooled with other threads and with the menus, and with no call chain.
- So stage shares below combine three sources: the module split (measured), the frame pairs (measured lower bounds),
  and the blocks attributed to stages through a static call graph of `SPEED2.EXE` (observed addresses, inferred
  grouping).

Scripts in `D:\debug_switch\carafe\tools\orchestrator\`: `exe-calls.sh` (linear disassembly of `.text` with
`llvm-objdump` in `carafe-runtime:latest`, the game folder mounted read-only; 1.24 M lines, 118712 calls, jumps and
returns to `exe\calls.txt`), `graph.js` (function starts = direct call targets; callers and callees),
`xdis.sh` (a range of the `.exe`), `exe-vcalls.sh` (calls through a vtable slot), `exe-refs.sh` (code that references
D3DX's strings), `stages.js` and `regions.js` (blocks by function and by the main-loop callees that reach them),
`windows.js` (`agg 4w "x86 frames"`).

### The frame loop (observed code, inferred meaning)

`0x5811c0` is called once from `0x580e70`; its loop runs from `0x5811e4` back from `0x581568` (`exe\mainloop.txt`).
Per iteration, in order:

1. Timers and messages: `0x43bdf0` (reads the performance counter; in the menus the thread spins in it and in
   `0x57ead0`, and with the system call path behind `QueryPerformanceCounter` that spin is most of the menu frame:
   `exe+0x3bde0…0x3be40` and `exe+0x17eb60` alone are 16 % of the menu samples, lines 844–938), two handler-list loops through `call *%esi`, `0x4c1210`, `0x600fa0`,
   `0x601020`, `0x57ee00`, `0x5eb390`, `0x57be30`, `0x456940`, `0x5f00f0`.
2. Simulation: `0x601a60` (frame time), `0x5809c0`, `0x5f0130`, then `0x609ce0( dt )` with `ecx = [0x890080]`; it
   loops over the cars (`[esi+0x1c+i*4]`, count `[esi+0x10]`) calling `0x5f6cd0`, `0x5f1160` and `0x5f5730`, and
   reaches the road and physics queries (`0x40cbc0`: a 2-D distance on a grid of 16-bit coordinates; `0x418620`).
   Then `0x578e10`. The other branches of the state machine (menus, loading) call `0x581050`, `0x5ea360`, `0x5f1390`.
3. Updates after the simulation: `0x60aee0`, `0x5663b0`, `0x600900`, `0x44efe0`, `0x483b40`, `0x572610`, `0x5728c0`,
   `0x5e79a0`, `0x57ef60`, `0x5ce850`, `0x5ce8a0`.
4. Render, `0x5d2850`: `IDirect3DDevice9::BeginScene` (vtable `+0xa4` at `0x5d2b10`), `0x5caad0` (at `0x5d2b55`: the
   views and passes; it calls the device directly through `+0xe4` SetRenderState, `+0x104` SetTexture, `+0x114`
   SetSamplerState, and the draw-list routine `0x5cf420` from 12 sites), `EndScene` (`+0xa8` at `0x5d2b70`),
   `Present` (`+0x44` at `0x5d2bc6`), then post-present work (`0x5d21f0`, `0x492e10`, `0x490020`, `0x5dbe80`).
5. After the render: `0x57a050`, `0x57a0e0`, `0x5f0140`, `0x570c00`, `0x5cec80` (the loop condition).

The draw list `0x5cf420` calls D3DX's Pass at `0x5cf78b` (frame pair `exe+0x26adb5<exe+0x1cf790`); Pass calls
ApplyPass `0x6691a1`, which loops over the pass's 0x64-byte state records into ApplyState `0x669286` (as found in the
game-speed study). The visibility test `0x5d6570` (T1) is called from `0x5d69b0`, which the recursive `0x5e99c0`
reaches through a pointer, so no direct-call path ties it to a main-loop callee; its place inside the render is
inferred from the region (`0x5c…–0x5d…` holds the render code) and from the frame pair `exe+0x1d6a74<exe+0x1e9a18`.

D3DX's code is `0x641000–0x6d1000`: all 73 references to its strings (`ID3DXEffectCompiler: …`, `$PRESHADER_PS`,
`D3DX_VERSION`) lie in `0x641793–0x6d0ba4` (`exe\d3dx-refs.txt`); the start-up table fill `0x6713a1` and the x87
matrix code near `0x671000` are inside. The lower edge is approximate (a few functions at `0x63c000–0x641000` may
belong to either side).

### Frame pairs in the race (measured, lower bounds)

`windows.js agg 4w "x86 frames"`, exp9 (exp7 in brackets):

| pair (inner < outer) | share | stage |
|---|---|---|
| `exe+0x269249<exe+0x26adb5` | 10.9 % (12.3) | ApplyState and the DXVK setters it calls, inside ApplyPass |
| `264a77`, `264a8f<269853`; `263baa`, `263b3b<264a77`; `260c3b<264a8f`; `269853<269249` | 12.4 % (12.0) | Evaluate: input loop, input copies, preshader VM, output conversion |
| `269f63<269249`, `26917e<269f63` | 3.6 % (4.6) | the shader, sampler and texture path of a pass |
| `exe+0x26d893<exe+0x1bcc94` | 2.9 % (3.4) | D3DX matrix multiply (x87) called by game render code `0x5bcc30` |
| `exe+0x1d6a74<exe+0x1e9a18` | 2.2 % (3.0) | visibility test in the scene walk |
| `exe+0x1cb541<exe+0x1d2b5a`, `1cf4cb<-`, `1cf553<-`, `26adb5<1cf790` | 1.4 % (1.4) | views and draw lists of the render |
| `exe+0x18140c<KERNEL32+0x24350`, `20a186<18140c` | 0.4 % (0.2) | directly under the frame loop |

D3DX effects at least 26.9 % (exp7 28.9 %), D3DX math at least 2.9 %.

### Blocks by stage (observed addresses, attribution by static reachability)

`regions.js`, main-thread game blocks of exp9 (26660 samples after the T1 filters, also removing the audio thread's
`0x720000–0x740000`); exp7 in brackets:

| group | share of hot game samples | biggest functions |
|---|---|---|
| D3DX effects (`0x641000–0x6d1000` less the math) | 51.4 % (55.1) | ApplyState `0x669286`, Evaluate `0x663a32`/`0x664a1f`, output conversion `0x660a42`, input copies `0x662ecc`, preshader VM `0x688e79`, shader path `0x668dfd`, `0x65ccc8`, `0x66c645` |
| game code reached only from the render | 9.1 % (9.2) | draw lists `0x5cf420`, `0x5c5020`, `0x5bd4a0`, `0x490920`, `0x5d6ad0` |
| game code reached from the render and from other main-loop callees except the simulation | 3.4 % (3.6) | `0x5bcc30` (calls the matrix multiply), `0x5bdde0`, `0x464c10` |
| game code reached from the render and from the simulation | 14.9 % (14.1) | `0x5bc300` (x87 vector code, from the render and from `0x60aee0`), `0x5c5930`, `0x440d40`, `0x5ceb30`, `0x4012b0` (all through render paths), `0x43c2b0` (through `0x609ce0`) |
| game code reached by no direct call | 7.0 % (7.3) | the visibility test `0x5d6570` and its caller `0x5d69b0`, `0x5d64d0`, `0x51a5d0` |
| C runtime (`_ftol2` at `0x75bed4`, math) | 6.1 % (6.6) | used by every stage |
| game code reached only from the simulation | 4.5 % (3.7) | `0x40cbc0`, `0x418620`, `0x5d3fb0`, `0x5d3330`, `0x5e72e0`, `0x5ae930` |
| D3DX math (x87 matrix code, `0x670000–0x672800`) | 3.0 % | `0x67192e` |
| other main-loop callees | 0.4 % | |

To shares of the main thread: game code is 77.3 % of it, the hot blocks cover 83 % of the game-code samples (26660
of 32200 outside the menu wait loops), and menu rendering is at most 2000 of those samples. D3DX effects are then
42.5 % of the game code (39 % if every menu sample were D3DX), 30–33 % of the main thread, plus whatever D3DX lies in
the 17 % of game code outside the hot blocks.

### The stages, as shares of the main thread

| stage | share | basis |
|---|---|---|
| Direct3D 9: DXVK's front end on the main thread (`SetRenderState`, `PrepareDraw`, state blocks, texture reference counts) | 17.5 % (16.5–18.6; exp7 19.3) | measured: `x86 by module` |
| D3DX effect application | 30–35 % | blocks; at least 26.9 % from frame pairs |
| game render code: scene walk, visibility, views, draw lists, per-object setup | 12–25 % | 12.4 % in blocks only the render or no direct call reaches; up to the shared and unattributed parts |
| D3DX x87 math and C runtime helpers | 6–7 % | blocks (1.9 % + 3.9 %); matrix multiply at least 2.9 % from frame pairs |
| simulation: cars, physics, road and world queries | 3–15 % | 2.9 % in blocks only `0x609ce0` reaches; the upper end assumes about half of the shared (9.5 %) and unattributed (13 %) time is simulation, which is not checked |
| updates between simulation and render | not separable | inside the shared and unattributed parts |
| timers, messages, input, Wine server and system calls | about 3.5 % | measured: pe 0.7 + native 1.1 + svc 1.7; `[SERVER]` 1.4 % of a frame for all threads |
| outside the 256 blocks | about 13 % | not attributed |

FEX's block entries (6.1 % of the main thread, `fex outside blocks`) are inside the rows above.

A cross-check from frame times: over the 16 intervals of 600 frames in the race (`[D3D9 calls]` lines 994–1988 with
the `[FPS]` lines between them), frame time = 13.8 ms + 7.05 µs × draws (r = 0.75; draws 600–1760 a frame,
18–28 ms). At the mean of about 1150 draws, 8.1 ms of a 21.5 ms frame grows with the number of draws: about 13300
cycles a draw for the game's per-object work, D3DX and Direct3D 9 together. The intercept is not "time that does
not depend on draws": the scene walk and per-pass work also grow with the scene but not in step with draws.

### Dependencies (inferred from the code and from Direct3D 9's rules)

- Timers and input → simulation: `dt` from `0x601a60` is the argument of `0x609ce0`.
- Simulation → updates → render: the render reads the cars and the world that the simulation wrote in the same
  iteration (the same world object, `[0x890080]`, is the `this` of the simulation). The game keeps one copy of this
  state, so within a frame the order is strict.
- Render → D3DX: the game sets effect parameters, then calls Pass for each draw; D3DX copies the dirty parameters into
  its own buffers (`0x662ecc`), evaluates the state expressions and sends every state to the device. Nothing flows back
  to the game.
- D3DX and render → Direct3D 9: setters return nothing the game uses. Points that need an answer or the caller's
  memory at call time, per frame in the race (`[D3D9 calls]`): buffer locks 4–246, draws from memory 165–322, state
  block applies 169–308 (DXVK applies them from its own copy), one `Present`. Get calls and queries are not counted.
- Direct3D 9 → `dxvk-cs` → GPU: asynchronous already (T2); the main thread's only wait there is 0.4 %.
- The audio thread is independent of the frame except for a Win32 mutex (about 55 `release_mutex` a frame; whose is
  not recorded).

### What could run in parallel or in a pipeline (inferred)

1. **D3DX and Direct3D 9 as one command producer on another core.** Together about 48–53 % of the main thread
   (D3D9 17.5 %, effects 30–35 %). The worker needs, per Pass, a copy of the effect's dirty parameters (the copy D3DX
   already makes), the draw-from-memory data at the call, and the game must get answers synchronously at locks without
   `DISCARD`/`NOOVERWRITE`, Get calls and queries. The main thread keeps the cost of recording.
2. **Render of frame N alongside the simulation of frame N+1.** Needs a snapshot of the state the render reads, taken
   at the end of the updates (step 3), and one frame of latency. The render path is 62–80 % of the thread and the
   simulation 3–15 %, so the overlap is bounded by the simulation's share; it pays mainly together with 1.
3. **The visibility walk as data-parallel work.** The recursive scene walk and its bit-table test (about 4 %) are per
   node and per object; they can be split across cores or prefetched (T1).
4. **The simulation alone** is too small to be worth splitting.

## What this means for the orchestrator

- The game's frame is mostly a render pipeline running on the CPU: D3DX effects and Direct3D 9 are half of the main
  thread, the game's own render code another eighth to quarter. The orchestrator's first lever is to move that
  command production off the main thread (W1, W3, W4), not to speed up the simulation.
- D3DX's effect application is pure: it reads copied parameters and writes only device state. That makes it the best
  candidate to move together with the Direct3D 9 front end, as one queue of Pass-level work items, rather than queuing
  thousands of state calls (W3 alone).
- Frame pipelining (W2) needs a snapshot of the render-visible state; its size is unknown and decides the cost.
- The stages are fixed in one function, `0x5811c0`, and each is a single call from it. The orchestrator can find
  such a loop in a game by the call that encloses `BeginScene`/`EndScene`/`Present` and the calls before it, and use
  the call sites as stage boundaries for profiling and scheduling (W12).

## Needs the console or the lab

1. **Stage tag in the profiler (lab, then console).** Walk the x86 EBP chain up to 32 frames, record the return
   address that falls inside a configured frame-loop function (here `0x5811c0–0x581575`) and whether a D3DX or
   `d3d9.dll` frame lies on the way. Gives inclusive shares per main-loop call site (simulation, updates, render) and
   splits D3DX and Direct3D 9 time by caller. It switches which pipeline split pays first: if the simulation plus
   updates are under 15 %, W2 waits behind moving the command producer (W1/W3/W4).
2. **Cross-stage data flow (lab).** Eden's dynarmic memory callbacks with the stage tag from 1: per frame, which cache
   lines or pages the simulation and the updates write and the render reads. Gives the size of a render snapshot. It
   switches whether frame pipelining (W2) is affordable: a snapshot of a few hundred KB is, tens of MB is not.
3. **Synchronous points per frame (DXVK counters, console).** Count buffer locks by flags, Get calls, queries, draws
   from memory and `Present` waits. It switches which calls a deferred Direct3D 9 and D3DX worker must keep on the
   main thread, and whether it must keep a copy of the device state for Get calls.
4. **D3DX call counts (profiler or an ASI counter, console).** Entries of Pass `0x66adb0`, ApplyPass `0x6691a1` and
   ApplyState `0x669286` per frame. Gives work items per frame and the share of Direct3D 9 state calls that come from
   D3DX. It switches the item size for 1 in W3/W4 (per Pass or per state).
5. **Race-only, main-thread-only block dump (profiler).** As in T1: removes the menu and pooling corrections from the
   table above.
