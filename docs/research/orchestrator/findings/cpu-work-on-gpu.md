# CPU work moved onto the GPU (W9)

## Answer

For Direct3D 9 games under DXVK the work that has a GPU form is already on the GPU: fixed-function vertex and pixel
processing become generated shaders, software vertex processing and `ProcessVertices` run as a GPU shader, format
conversions run as compute shaders, `D3DUSAGE_AUTOGENMIPMAP` uses GPU mip generation, queries are GPU queries. What
stays on the CPU is control work: the game's simulation and culling, D3DX re-applying effect states, DXVK's state
tracking, Vulkan command recording, NVK writing push buffers. None of it has a GPU equivalent, because its output is
API state that configures the GPU, or data that lives in the game's CPU memory. The generic candidates are small: the
DrawPrimitiveUP copies and shader-constant copies, which must stay on the CPU (the data is the game's, valid only during
the call) and cost the main thread at most a few percent (DXVK's out-of-line `memcpy` is 0.2–0.3 %); device-generated
commands, which NVK exposes but which would only move `dxvk-cs` work, already off the main thread; D3DX preshaders
folded into shaders, the one real GPU offload, large work with a gain nobody has measured. Game-specific CPU work
(software skinning, particles, CPU culling) cannot be found generically. So W9 is worth at most 1–3 % of NFSU2's
main thread. The GPU's idle time (27–54 % in the race) is better kept as headroom: it is what lets CPU-side mechanisms
turn into frames up to the GPU's ceiling of about 83 fps ([gpu-submission.md](gpu-submission.md)).

## Evidence

Trees as in [shared-memory-attributes.md](shared-memory-attributes.md): DXVK `b1a1c99`
(`carafe/runtime/build/dxvk-src`), mesa-switch `d4a00ea0ab` (GitHub), Autorun `636913ca`. Logs:
`D:\debug_switch\carafe\nfsu2-exp9\logs`.

### Already on the GPU in DXVK

- Fixed-function pipeline as generated shaders: `src/d3d9/d3d9_fixed_function.cpp`.
- Software vertex processing and `ProcessVertices`: `D3D9SWVPEmulator`, a GPU shader module
  (`src/d3d9/d3d9_swvp_emu.h:44-48`); SWVP buffers get storage-buffer usage for it (`src/d3d9/d3d9_common_buffer.cpp:109-113`).
- Format conversions for formats Vulkan lacks: `D3D9FormatHelper`, compute (`src/d3d9/d3d9_format_helpers.h:10`).
- Mip generation: `ctx->generateMipmaps` (`src/d3d9/d3d9_device.cpp:5513`).
- NFSU2 uses vertex shaders (1400–2240 vertex shader constant sets a frame, `[D3D9 calls]` in
  `logs/autorun_runtime.log`), so its vertex work is GPU work already.

### What stays on the CPU, and where

- Main thread: the game's code 75–77 %, about half of it the D3DX linked into `SPEED2.EXE` re-applying effect states;
  `d3d9.dll` 17–19 % (`PrepareDraw`, state blocks, state setters, texture reference counts); FEX block entries 6–7 %
  ([../../game-speed/REPORT.md](../../game-speed/REPORT.md), lines 193-197, 230-238).
- `dxvk-cs`: `d3d9.dll` 21–24 %, native NVK and Wine 19–28 %, waiting 31–57 % (`[PROF] 44w`).
- Copies DXVK makes on the main thread: DrawPrimitiveUP vertex data into the UP ring (`d3d9_device.cpp:3176-3224`,
  `FillUPVertexBuffer` in `d3d9_device.h:1360-1375`), up to 260 such draws a frame; shader constants for each draw with
  dirty constants (`UpdateShaderConstants`, `d3d9_device.cpp:6068-6160`, streaming stores in
  `d3d9_constant_copy.cpp:297-317`). The out-of-line `memcpy` lands in `ucrtbase.dll`, 0.2–0.3 % of the main thread in
  the race; the constant loop is inside `d3d9.dll`'s share and not separated.

### The generic candidates

| Candidate | GPU form | Why it does or does not move | Gain on NFSU2's main thread |
|---|---|---|---|
| DrawPrimitiveUP copies | GPU reads the game's vertices in place (`VK_EXT_external_memory_host`, which the Switch backend supports: `nvkmd/switch/nvkmd_switch_pdev.c:473`, `nvk_physical_device.c:259`) | the pages are CPU-cached game memory: each draw would need a cache clean of the range (about the cost of the copy), an NvMap per imported range, and the pointer is valid only during the call | none; the copy is ≤0.3 % |
| Shader constant uploads | the shader reads D3D9's constant array directly | the GPU reads at execution time, the game changes constants between draws, so each draw needs its own copy anyway; that copy is the upload | none from the GPU; a cheaper copy is a CPU question (C7) |
| State validation, command recording | `VK_EXT_device_generated_commands`, enabled by NVK for Maxwell B and newer (`nvk_physical_device.c:248, 656-664`) | DXVK still has to parse every D3D9 call on the CPU; DGC would move only `dxvk-cs` recording, already on core 3 | none on the main thread |
| D3DX effect state application | none | its output is render, sampler and texture-stage states: API state, not data | none; this is CPU work to drop, not move (W3, W4) |
| D3DX preshaders (expressions that compute shader constants from effect parameters) | fold the expression into the vertex or pixel shader | the expressions are bytecode for D3DX's own VM (Wine's version: `external/autorun/dlls/d3dx9_36/preshader.c:1232-1722`); moving them means recognising the game's linked D3DX, rewriting its shaders and passing raw parameters | unknown; bounded by the VM's share of the main thread, not yet measured |
| D3DX math (`D3DXMatrixMultiply`, 1–3 %) | compute on the GPU | the results feed the game's own CPU code (culling, physics) as well as the GPU | none |
| Software skinning, particles, CPU culling | shaders, compute | game-specific code paths; nothing generic identifies them. D3DX's `UpdateSkinnedMesh` is the one generic entry point, and NFSU2 does not import `d3dx9_*.dll` (`d3dx9_43.dll` 0–0.1 %) | not applicable |
| Texture decoding and mip filtering in D3DX at load time | upload compressed, GPU mip generation | loading time only | none on frames; possibly on loading (W11) |

### GPU headroom

- `gpu busy` 46–73 % in the race; fps ÷ busy gives a GPU-bound rate of 61–107 fps, median 83, at 614 MHz GPU and
  1331 MHz memory ([gpu-submission.md](gpu-submission.md)). At stock handheld GPU clocks the ceiling is lower.
- Every CPU-side speedup of the main thread up to about 1.75× turns into frames only while this headroom lasts; work
  moved onto the GPU spends it.

## What this means for the orchestrator

- W9 is not a stage of its own for Direct3D 9 games. The orchestrator should not plan GPU offload for NFSU2-class games;
  its CPU work belongs on other CPU cores (W1, W3) or removed (W4).
- The GPU's headroom is a constraint the closed loop tracks, not a resource it fills: when fps ÷ busy approaches the
  frame rate the CPU mechanisms deliver, further CPU gains stop paying, and then the right moves are on the GPU side
  (fewer GPU passes the player does not see), which are per-game profile decisions.
- The one generic GPU offload with possible weight is D3DX preshaders. It is worth a feasibility study only if the
  preshader VM shows up as several percent of the main thread in the existing block dump.
- Games that use software vertex processing are a special case already handled: DXVK runs it on the GPU.

## Needs the console

Nothing new is needed on the console for W9's decision; two numbers come from files already taken, one from a counter
proposed in [shared-memory-attributes.md](shared-memory-attributes.md).

| Measurement | How | Decision it switches |
|---|---|---|
| Share of the main thread in D3DX's preshader VM | offline: find the VM's dispatch loop in `SPEED2.EXE` (opcode table as in Wine's `preshader.c`), sum its blocks in `logs/jit-blocks.txt` and the `[PROF] 4w x86` buckets | whether folding preshaders into shaders is worth a study (above about 3 %) |
| Share of the main thread in DXVK's constant and UP copies | offline: symbolise the `d3d9.dll` offsets in `[PROF] 4w x86` with a link map of the i386 `d3d9.dll` built in `runtime/build/dxvk/i386` | whether copies matter at all; with C7's `uc-write`, whether to change their memory type |
| Bytes copied per frame (UP vertices, constants) | the `[D3D9 calls]` counters listed in [shared-memory-attributes.md](shared-memory-attributes.md) | the same, and the per-game profile |
