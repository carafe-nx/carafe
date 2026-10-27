# W4. The D3DX linked into a game's `.exe`: finding it, running it natively, running it on other cores

## Answer

**Which games.** D3DX 9 was a static library up to the December 2004 DirectX SDK. From February 2005 it shipped as
`d3dx9_24.dll` through `d3dx9_43.dll`. D3DX 8 was always static. So the Direct3D 9 games of 2003 to early 2006 carry
D3DX inside their `.exe`, and later ones import a DLL that Autorun already provides as Wine's builtin. Of the four
local Direct3D 9 games, NFSU (2003) and NFSU2 (2004) link it in. NFS Carbon (2006) imports `d3dx9_30.dll` and Dead
Space (2008) `d3dx9_38.dll`.

**Finding it** needs no knowledge of a particular game. Static D3DX has three markers:

- its CPU-dispatch routine: the same `cpuid` sequence and table writes, found at the same place in its code in NFSU
  and NFSU2;
- the effect compiler's error strings;
- the shader compiler version strings.

**The cost in NFSU2's race.** The effect code takes at least 27 % of the main thread, DXVK's setters below it
included. D3DX's x87 `D3DXMatrixMultiply` takes about 3 % more. Four steps, by gain per risk:

| Step | What it does | Main thread | Risk |
|---|---|---|---|
| 1 | Point the dispatch table's matrix-multiply slot at an SSE function | about 1.03× | low; generic for static D3DX 9 |
| 2 | Drop unchanged effect states where D3DX's own loop sends them, before the indirect call to the device | 1.03–1.06×, if W3's shadow state is there to keep the filter safe | — |
| 3 | Skip D3DX's per-state work: unchanged static states, and expressions whose inputs did not change, as Wine's `update_version` does | 1.15–1.25× | needs the internals of each D3DX release; about seven static releases exist |
| 4 | Run effect application on the W3 worker, with a snapshot of the parameters changed since the last pass | up to 1.25–1.35× together with W3 | needs a free core; highest risk |

**Native ARM64 code** through FEX's own thunk opcode pays only for functions that run long per call: an `ApplyPass`,
not a matrix multiply. A 74-instruction SSE multiply translated by FEX already beats native code plus a crossing.
Native code also cannot call back into translated DXVK, so a native `ApplyPass` can only write into W3's ring.

## Evidence

### Markers in the executables (checked on the local copies)

These were read with a small capstone script over the PE files in `D:\Game`, not run.

**Imports:**
- `SPEED2.EXE` imports `d3d9.dll` and no D3DX DLL;
- `Speed.exe` (NFSU) imports `d3d9.dll` and no D3DX DLL;
- `NFSC.exe` imports `d3dx9_30.dll`;
- `Dead Space.exe` imports `d3dx9_38.dll`.

**Strings:**
- `SPEED2.EXE` holds 56 `ID3DXEffectCompiler: …` error strings, for example "ID3DXEffectCompiler: Unexpected
  component type" at `0x7aec28`. These belong to the effect compiler, so the whole effect framework is linked in. It
  also holds the state names of the effect parser (`ALPHABLENDENABLE`, `ZWRITEENABLE`, `MINFILTER`) and the shader
  compiler versions of its compiled shaders: 317 × "D3DX9 Shader Compiler 5.04.00.2904" and 30 × "4.09.00.1126".
- `Speed.exe` has 54 `ID3DXEffectCompiler` strings.
- `NFSC.exe` has only "9.04.91.0000" in its compiled shaders and no compiler strings.

**The CPU dispatch in `SPEED2.EXE`:**
- `0x6712bb` runs `cpuid` 0, 1, 0x80000000 and 0x80000001 and builds a feature mask: MMX `0x20`, SSE `0x40`, 3DNow!
  `0x80`, extended `0x100`/`0x200`.
- `0x6713a1` calls it, then fills a table of function pointers passed in `[ebp+8]`:
  - with 3DNow! (`test al,0x20; test al,al; jns`) slots `0x0`–`0x114` (`0x6713c1` onward);
  - with SSE only slots `0xf0`–`0x114` (`0x6715e1`–`0x67162b`);
  - the x87 defaults are written at `0x671aa3`, slot 0 = `0x671637`.
- `0x671637` takes `out, a, b` from `[ebp+8], [ebp+0xc], [ebp+0x10]` and runs `fld [eax]; fmul [edx]; …`: it is
  the x87 `D3DXMatrixMultiply`. Slot 0 is therefore the matrix multiply, and the SSE path never replaces it, as
  `docs/research/game-speed/REPORT.md` says.
- `Speed.exe` has the same routine at file offset `0x1d6989` (`0x5d6989`): the same instructions and masks, an older
  compiler without `mov edi, edi`. Its table init follows at `0x5d69e4` with the same slot layout from `0x0` to
  `0xb0`.
- The byte pattern `b8 01 00 00 80 0f a2 b9 00 00 00 80 23 ca` (`cpuid` 0x80000001 and its 3DNow! test) appears once
  in each of the two static-D3DX games and in neither DLL-D3DX game.

**Calls into the device.** `SPEED2.EXE` has 87 `call [reg+0xe4]` (SetRenderState) sites, 55 `[reg+0x114]`
(SetSamplerState), 25 `[reg+0x10c]` (SetTextureStageState), 39 `[reg+0x104]` (SetTexture) and 13 `[reg+0xb0]`
(SetTransform). Some of them belong to the game's own code.

**The effect magic.** The fx_2_0 magic `0xFEFF0901` does not appear as an immediate in either NFS executable. The
effect loader has to be found another way, for example by the code that refers to the `ID3DXEffectCompiler` strings
(not done).

### What the effect code costs (console)

`D:\debug_switch\carafe\nfsu2-exp9\logs\autorun_runtime.log`, race interval at line 1041 (`[PROF] 4w x86 frames`,
96–106 s). The top 12 frame pairs only, so the sum is a lower bound. DXVK keeps no frame pointer, so its setters'
time counts to the D3DX frame that called them. Call chain and roles from the cloud study
`D:\debug_switch\carafe\cloud\results-d3dx\files\d3dx-effects\REPORT.md`:

| Part | Frame pair | % of main thread |
|---|---|---|
| ApplyState, with the DXVK setters it calls | `269249<26adb5` | 10.5 (9.4–12.1 over the race intervals) |
| Evaluate: self and input loop | `264a77<269853`, `269853<269249` | 4.9 |
| Evaluate: input copies | `263b3b<264a77` | 2.2 |
| Evaluate: preshader VM | `263baa<264a77` | 2.0 |
| Evaluate: output conversion | `264a8f<269853`, `260c3b<264a8f` | 3.6 |
| Shader and sampler path | `269f63<269249`, `26917e<269f63` | 3.6 |
| **Effect total** | | **≥ 26.8** |
| D3DXMatrixMultiply (x87), from game code | `26d893<1bcc94` | 3.2 (2.1–3.9) |

The cloud study measured 22.2 % in an earlier run (36–46 fps) with the same pairs. The rise follows the counts: up to
17738 render states a frame in this run (line 1329).

**Per state,** counted in the cloud study (`REPORT.md` table, FEX `395b132f` + patches, A57 setup):
- D3DX's own side (the loop, ApplyState up to its switch, the render-state case, the tail): 61 x86 and 90 ARM64
  instructions;
- DXVK's unchanged path: 25 ARM64 with Carafe `0004`.

**Per matrix multiply:** x87 312 x86 and 1616 ARM64, an SSE version 72 and 74.

D3DX calls the device directly, through its vtable, with no state manager and no comparison with the last value sent
(cloud study, "Proven").

### Wine's D3DX, for comparison

- Wine's `d3dx9_36/effect.c` (current master, read through the GitHub mirror):
  - `d3dx9_apply_pass_states(effect, pass, update_all)` gets `new_update_version = next_effect_update_version(effect)`
    and calls `d3dx9_apply_state` for each state;
  - `d3dx9_apply_state` returns early when neither `update_all` nor the parameter is dirty;
  - device calls go through `SET_D3D_STATE`, which uses the effect's state manager when there is one.

  `BeginPass` passes `update_all` true and `CommitChanges` false (the cloud study and the game-speed report; the
  lines for those two calls were not re-read in this pass). So Wine skips only parameter-driven states that did not
  change between `CommitChanges` calls. On `BeginPass` it re-sends every state, as the static D3DX does.
- Autorun ships Wine's builtin `d3dx9_24.dll`–`d3dx9_43.dll` (`external/autorun/horizon-dlls/switch/wine/drive_c/
  windows/system32/`). They come prebuilt; Carafe's `runtime/Makefile:42-51` configures Wine's PE tree, i386
  included, but builds only its headers.

### FEX's way to call native code

- **The thunk opcode** `0F 3F` + SHA-256 decodes to `ThunkOp` (`runtime/build/fex-src/FEXCore/Source/Interface/Core/
  OpcodeDispatcher.cpp:70-87`), which emits the `Thunk` IR op. Its lowering
  (`FEXCore/Source/Interface/Core/JIT/BranchOps.cpp:334-362`):
  - spills the static registers and pushes the dynamic ones;
  - calls the host function with the context and a guest argument pointer (ECX on 32-bit);
  - restores the registers;
  - returns through the guest stack.

  There is no unix-call dispatcher and no lock, so it should cost less than the 158–175 instructions of a unix call.
  How much less is not counted.
- **Off on Windows.** The opcode's table entry is compiled out under `_WIN32`
  (`OpcodeDispatcher/SecondaryTables.h:145-149`), and no WoW64 code calls `SetThunkHandler` (only the Linux front
  ends do: `Source/Tools/FEXInterpreter/FEXInterpreter.cpp:563`).
- **Enabling it** is a FEX patch, which Carafe can carry (decision 29). It would:
  - drop the `#ifndef`;
  - give WoW64 a handler that maps hashes to functions in an ARM64 PE;
  - add a guest stub `0F 3F <hash>; ret` per replaced function.

### Moving D3DX to another thread

- `ApplyPass` reads the effect object, which the game changes between passes with `SetMatrix` and similar calls.
  Running it on another thread needs a copy of the parameters changed since the last pass, taken on the game thread.
- It calls the device. On the W3 worker that is the device's own thread, so no lock is needed.
- No public layout of the effect object exists for the 2003–2004 releases. The 0x64-byte state records and the
  dynamic flag at `[rec]` are known from NFSU2 only (cloud study), not verified on NFSU.

## Design, by step

1. **SSE matrix multiply.**
   - Carafe's builder finds the dispatch init by the byte pattern above and the x87 default initializer (`0x671aa3`
     in NFSU2) by its `mov [eax], imm32` run. It stores the address of the slot-0 write in the NSP's per-game data,
     as the result of a generic scan, not as code for one game.
   - At start-up an i386 helper loaded before the game's entry point rewrites that immediate (and the 3DNow! one) to
     point at an SSE `D3DXMatrixMultiply` that adds the products in the x87 order ((p0+p2)+(p1+p3)). The proxy
     `d3d9.dll` of W3, or a small `carafe-hooks.dll`, can be that helper: the game imports `d3d9.dll`, so its
     `DllMain` runs first.
   - FEX has translated nothing yet at that point, so the change is not self-modifying code.
   - Gain: 3.2 % × (1 − 74/1616) ≈ 3 %, so about 1.03×. Other table slots (vector transforms) can follow once the
     lab identifies them by calling each slot with test inputs.
2. **Filter at D3DX's call sites.**
   - Each `call [reg+0xe4]` and its neighbours inside D3DX's ApplyState switch becomes a direct call to the W3
     proxy's compare.
   - That removes the indirect branch through FEX's lookup and the DXVK early-out: about 25 ARM64 plus 15–25 for the
     call and return, against 10–15 for a direct call into the shadow compare.
   - Arithmetic: 41441 unchanged states a frame × 30–40 instructions saved = 1.2–1.7 M instructions, or 1.7–2.4 M
     cycles at IPC 0.7, which is 4–6 % of a 41.9 M-cycle frame. The low end allows for some states going through
     state blocks rather than D3DX: 1.03–1.06×.
   - Safe only with W3's shadow. Every write to the device goes through the proxy, state block applies included, so
     the shadow is the device state. A shadow without W3 would need every device write to invalidate it, which is
     the risk the cloud study names (its option 5).
   - The call sites are found by their return addresses: the proxy can histogram the callers of each setter for a few
     frames (measurement below).
3. **Skip D3DX's own work.** Replace ApplyState's loop for one D3DX release:
   - a static state (`[rec] == 0`) whose value equals the shadow costs a load and a compare;
   - a dynamic state runs Evaluate only when one of its inputs changed. A version counter is bumped by the effect's
     `Set*` methods, which are found through the effect's vtable, the way Wine's `update_version` works. Its value
     is compared with a stamp kept per expression.

   The arithmetic:
   - the 26.8 % effect total minus DXVK's unchanged path below it (about 1.3 M of the 4.4–5.0 M instructions
     `d3d9.dll` runs, roughly 5 % of the main thread) leaves about 22 % that is D3DX's own;
   - if 96–99 % of states and most expressions do not change between draws, 70–85 % of that goes;
   - so 15–19 % of the main thread, 1.18–1.23×, and up to 1.25× with step 2.

   This needs the layout of each static D3DX release: 9.0, 9.0a, 9.0b, 9.0c Summer 2004, October 2004 and December
   2004, plus D3DX 8. Seven implementations, each tied to a library release rather than to a game.

   **Alternative:** route the static D3DX's effect entry points to Wine's i386 `d3dx9_36` (prebuilt in Autorun),
   patched in one file (`effect.c`) to compare with the last value sent. One implementation then serves the static
   and the DLL games alike. Risk: Wine's effect code differs in behaviour from a 2004 D3DX; not verified on any game.
4. **On another core.**
   - With W3 in place, `Pass` becomes one ring record: the effect, the pass, and a copy of the parameters changed
     since the last record (typically one to four matrices, 64–256 bytes).
   - The worker runs the evaluation and the state application against its own copy of the parameter block.
   - Gain: the main thread loses most of the effect's 22–27 % but pays for the copy (≤ 2 %). That is 1.25–1.35×
     together with W3, if the worker, now 35–45 % of a core, keeps up.
   - It needs step 3's knowledge of the effect object, so it comes after step 3.

**Native code.**
- With the FEX thunk enabled, a native step-3 loop (states and the preshader VM in ARM64) would cost about half of the
  translated one. It must write its device calls into W3's ring, since it cannot call translated DXVK.
- Worth it only after step 3 has shown how much of the effect time is left.
- A native `D3DXMatrixMultiply` is not worth a thunk: an estimated 60–100 instructions of crossing plus about 40 of
  native work against 74 translated SSE instructions.

## What this means for the orchestrator

- **At NSP build time** the orchestrator's data for a game includes:
  - whether D3DX is linked in, from imports and markers;
  - the dispatch table site;
  - the D3DX release, from the compiler version strings and the table layout.

  All of it comes from a generic scan in `carafe-core`.
- **At start-up** it chooses the steps for that release. At run time it reads the proxy's per-caller counts to
  confirm that the hot call sites are inside D3DX.
- **Steps 2 and 4 depend on W3.** W3 is the stage before them in the plan.
- **For DLL-D3DX games** (2005 onwards), the same filter goes into Wine's `d3dx9_36` build. These games use Wine's
  builtin through the version-named DLLs.

## Needs the console or the lab

| Measurement | How | Decision it switches |
|---|---|---|
| Library scan | `carafe-core`: imports, `ID3DXEffectCompiler` strings, compiler version strings, the `cpuid`/3DNow! pattern, over every game in the owner's library and the lab's | How many games steps 1–3 reach; which D3DX releases need step 3 first |
| Table slot identity | Lab: an i386 probe calls every slot of the dispatch table with test vectors and matrices and names the D3DX function it matches | Which slots step 1 can replace beyond the matrix multiply |
| Callers of the device setters | Proxy (or DXVK `0005`): histogram of return addresses per setter for 600 frames, top 16 into the log | Which call sites step 2 rewrites; confirms the sites are D3DX's |
| Effect counters | i386 hook on ApplyState (lab first): states per pass, static and dynamic, how many equal the last value sent, Evaluate calls with no changed input | The real gain of steps 2 and 3 (the 70–85 % assumption) |
| SSE multiply on the console | Step 1 on and off, same track: `[PROF]` share of the `26d893<1bcc94` pair and `[FPS]` | Ship step 1 |
| FEX thunk crossing | `[A57]`-style microbenchmark: an empty native function through `0F 3F` with the FEX patch, ns per call | Whether native code is ever worth it for D3DX's leaf functions |
| Wine effect on a static-D3DX game | Lab: NFSU and NFSU2 with their effect entry points routed to Wine's `d3dx9_36` | Whether step 3 can be one implementation instead of seven |
