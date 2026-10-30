# DXVK-Sarek and Carafe's DXVK

What DXVK-Sarek does that Carafe's DXVK does not, what was carried over, and what each piece is worth.

## Why

A tester ran Dark Souls II with Carafe 0.4.0 and with an experimental 0.5.0 build over the same three places. The
two logs (handheld, CPU at 1963 MHz in both):

| | 0.4.0 | 0.5.0 experimental |
|---|---|---|
| DXVK | `v1.10-async`, chosen by the tester in 0.4.0's settings | Carafe's 3.1.1, the only one since [decision 43](../../decisions/0043-one-path-for-every-game.md) |
| Frames over the run | 16 265 in 345 s, about 47 fps | 14 934 in 367 s, about 41 fps |
| Game thread (`4w`) | 95–98 % busy | 45–58 % busy at 26–36 fps: it waits |
| Shader compiler threads at the worst moments | — | `dxvk-shader-l` 36 % |
| Writes to the SD card | 447 | 1 460 (DXVK 3's shader cache) |

The tester saw long hitches while shaders load in 0.5.0, half-second ones in 0.4.0, and 5–10 fps less in 0.5.0 even
with their own DXVK dropped into the game folder. The 0.5.0 build they ran predates the FEX work of stage 4 and the
cached GPU command memory, so the second gap is not measured on the current runtime yet.

The tester was pointed at [DXVK-Sarek](https://github.com/pythonlover02/DXVK-Sarek), a fork of DXVK 1.10 for GPUs
without Vulkan 1.3, as doing better than Carafe's build.

## Method

Sarek forks upstream at `v1.10.3`; its own history is `v1.10.3..52a1b172` (568 commits, 539 without merges). Every
commit was matched line by line against upstream 3.1.1 (`b1a1c99`), and every commit that could change speed was read
by hand together with the 3.1.1 code it would land in. Line numbers below are in pristine 3.1.1.

| Kind | Commits | Not in 3.1.1 |
|---|---|---|
| Speed | 59 | 20, forming four ideas (below) |
| Smoothness (pacing, latency) | 6 | 1, the limiter's methods and pacing |
| Compatibility | 263 | 28, none needed on the Switch: D7VK/DirectDraw 17, Mali/Adreno/MoltenVK 11 |
| Build and other | 79 | — |
| Per-game profiles in `config.cpp` | 132 | — |

Most of Sarek's 2025–2026 work backports upstream 2.x–3.x code into 1.10: the page and pool allocator, CS chunk
batching, 32 queued command buffers, the skipped CS flush, `UnlockBuffer` without an implicit flush, the D3D9
hot-path fixes, the D3D8 front end, the Win32 WSI layer. 3.1.1 has all of them. Its 37 shader compiler fixes target
`dxso`/`dxbc`, which 3.1.1 replaced with `dxbc-spirv`.

## Sarek's own ideas

1. **Background pipeline compilation** (`dxvk.shaderCompilationMethod`, `dyasync` by default). In 1.10 a missing
   pipeline is compiled on the CS thread at draw time. `async` skips the draw and compiles in the background;
   `dyasync` compiles the first variant of a shader set at once and later draws with the last variant compiled for
   the same shaders and render pass while the right one compiles.
2. **All workers take every job** (`DXVK_ALL_CORES`).
3. **No repeated pipeline binds**: a pipeline already bound is not bound again (`2df61a47`).
4. **Frame rate limiter methods and pacing** (`DXVK_FRAME_RATE_METHOD`, `DXVK_FRAME_RATE_PACING`, `5797fb9b`).

Also in Sarek, and already in 3.1.1 in another form: low-latency frame pacing (3.1.1: `dxvk.latencySleep`, the
built-in tracker, `dxvk_device.cpp:587-601`), and `d3d9.cachedDynamicBuffers` (3.1.1:
`d3d9.cachedWriteOnlyBuffers`, `d3d9_options.cpp:67`).

## Where 3.1.1 still stalls a frame

3.1.1 answers Sarek's problem with graphics pipeline libraries: on a missing pipeline it links a base pipeline from
precompiled shader libraries and queues the optimized one at low priority (`dxvk_graphics.cpp:1086-1116`). The
Switch's NVK supports them (`Graphics pipeline libraries supported` in every log). The CS thread still compiles in
full while drawing in two cases:

- **The state cannot be linked** (`canCreateBasePipeline` false, `dxvk_graphics.cpp:1228-1338`): non-fill polygon
  mode, inputs the vertex shader reads but the layout lacks, dual-source blending, flat shading, swizzled render
  targets and a few more.
- **A shader library is not built yet** (`dxvk_shader.cpp:214-225`). On the Switch this is the common case, found
  while reading the code rather than in Sarek: 3.1.1 tracks pipeline lifetime on a 32-bit process with any driver but
  RADV (`dxvk_device.cpp:133-151`). Libraries that workers build ahead are destroyed at once
  (`dxvk_shader.cpp:251-257`) and built again on the CS thread at first use, to save address space. Under WoW64 the
  driver lives in the 64-bit host, so the Vulkan pipelines never used the game's 4 GB. The setting
  `dxvk.trackPipelineLifetime = False` turns it off without a patch.

On three cores 3.1.1 starts three compiler workers, but only the first takes normal and low-priority jobs — the
libraries and the optimized pipelines — and the other two take high-priority jobs only
(`dxvk_pipemanager.cpp:104-117`).

The limiter and the latency sleep both wait through `Sleep::sleep`, which busy-waits the last 4 ms plus a sixth of the
wait (`util_sleep.cpp:71-107`); Wine on the Switch reports 1 ms timer resolution.

## Carried over

Each piece is its own key; the console's settings menu offers the keys through Autorun's DXVK list (Autorun patch
`0082`). Background pipelines and every worker taking every job are on by default (`0022`); the rest are Experimental
rows, off.

| Patch | Key | What it does |
|---|---|---|
| `0016` | — | Counts pipelines built while drawing: every ten seconds a `[DXVK pipelines]` line with how many (linked or in full), their time, the longest, shader libraries built while drawing and draws skipped. |
| `0017` | `dxvk.shaderCompilationMethod` = `async`, `dyasync` | `async`: a draw whose pipeline cannot be linked at once from ready libraries is skipped and the pipeline is compiled in full on a high-priority worker. `dyasync`: the first pipeline of each shader set is built while drawing, later ones as with `async`. |
| `0018` | `dxvk.skipRedundantPipelineBinds` | The command list remembers the graphics pipeline bound to the execution buffer and does not bind it again; it forgets it on a new buffer, a secondary buffer begun or ended, and executed secondaries. |
| `0019` | `dxvk.frameRateMethod`, `dxvk.frameRatePacing` | Sarek's limiter: `timeline` (3.1.1's), `deviation` (DXVK 1.x's), `reactive`; waits `precise` (3.1.1's), `sleep`, `sliced`, `spin`. |
| `0020` | `dxvk.allCompilerWorkers` | Every compiler worker takes every job, as `DXVK_ALL_CORES`. |
| `0021` | — | The `[DXVK pipelines]` line also counts lookups that handed out a linked pipeline, which the GPU runs slower, and the compiler jobs waiting. |
| `0022` | — | `async` and every worker taking every job become the defaults. |

Not carried over:

- **Drawing with a neighbouring variant** (the stand-in of `dyasync`). In DXVK 3 the linked base pipeline is that
  stand-in, built for the right state. A variant built for other blend, depth or vertex state would also break
  Vulkan's rules: DXVK sets dynamic state for the pipeline's own state vector (`dxvk_context.cpp:6849-6900`). A
  deferred draw is skipped instead.
- **Sarek's allocator changes, CS and submission changes, shader compiler fixes**: in 3.1.1 already, or in code 3.1.1
  replaced.
- **D3D3–7 (D7VK), Mali, Adreno and MoltenVK support, per-game profiles**: compatibility outside Carafe's targets.
- **`d3d9.cachedDynamicBuffers`**: 3.1.1's `d3d9.cachedWriteOnlyBuffers` is the same switch. On the Switch it changes
  nothing: DXVK needs host-coherent memory, NVK's only cached type is not coherent (`Type 0: DEVICE_LOCAL |
  HOST_VISIBLE | HOST_CACHED`), so the request lands in the uncached type (`dxvk_memory.cpp:2182-2212`).

## Results

The lab measured keys against the same runtime with them off, on NFSU and NFSU2 (Dead Space cannot be packed there:
one of its paths is longer than hacBrewPack reads). Two runs of the same build differed by about 10 % in fps, so a key
needs several pairs; the main thread's time a frame on a standing race start varies about 1 ms.

| Key | Pairs | Result |
|---|---|---|
| `shaderCompilationMethod = async` | 2 (NFSU2) | Pipelines built while drawing 19 → 0, about 400 draws skipped while they compiled; the race picture unchanged; main thread 26.7 → 26.4 ms a frame. |
| `shaderCompilationMethod = dyasync` | 1 | Builds while drawing 19 → 11; no verdict on speed. |
| `allCompilerWorkers = True` | 1 | Time building pipelines while drawing 782 → 191 ms; main thread 27.0 → 25.5 ms. |
| `trackPipelineLifetime = False` | 3 (+1 NFSU) | NFSU: libraries rebuilt while drawing 309 → 78 ms. NFSU2: no gain beyond the noise. |
| `enableGraphicsPipelineLibrary = False` | 1 | Builds while drawing take longer (689 → 1 800 ms); no verdict. |
| Redundant binds, limiter method and pacing | — | Not measured on their own. |

The console (Carbon, every Sarek row on, two races) built no pipeline while drawing; the first, cold run skipped about
1 200 draws in its first 40 s, the second one. The compiler workers took under 1 % of a core each, and the orchestrator
did not move them: they keep affinity 0x7 at the lowest priority, and the game thread stayed at 90–93 % of core 0. The
main thread took 16.5 ms a frame, against 16.85 ms for the best run of the build before, within the spread.

What became of each key is [decision 53](../../decisions/0053-background-pipelines-built-in.md).
