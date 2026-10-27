# W3. The Direct3D 9 front end on another core, in batches, with unchanged states dropped on the game's side

## Answer

The boundary goes at the COM interface: an i386 proxy `d3d9.dll` stands between the game and DXVK.

**On the game thread, the proxy:**
- keeps a shadow copy of the device state;
- drops a setter that leaves its value as it is, as cheaply as DXVK's early-out does today (about 25 ARM64
  instructions);
- answers `Get*` calls from the shadow;
- appends everything else to a single-producer ring.

**On a free core,** a Wine worker thread replays the ring into today's DXVK. DXVK's own `dxvk-cs` thread stays
behind the worker. The pipeline becomes game → worker → `dxvk-cs` → GPU.

**The gain.** In NFSU2's race `d3d9.dll` is 16–18 % of the main thread. By the instruction-count model below, the
proxy keeps 30–50 % of that cost on the game thread, so the main thread runs **1.09–1.15× faster**: about 47 → 51–54
fps median at 1887 MHz. The GPU, 46–73 % busy, has room for it. The gain holds only if three conditions are met:

1. **No more than a handful of synchronous drains per frame.** The skeleton makes every state block `Apply`
   synchronous, and the race makes 300 of them a frame. Applies must be queued, with the proxy mirroring what each
   block holds.
2. **Unchanged states are dropped before they are queued.** Queuing costs 54 instructions against 25. Without the
   filter the 41 000 unchanged calls a frame add about 1.2 million instructions, and the gain falls to about 1.05×.
3. **The worker has 12–16 % of a core** that the main thread is not on. Cores 0 and 2 are about 90 % idle in the race.

Dropping states on the game's side saves nothing by itself, because DXVK `0004` already returns before the lock. The
work the proxy moves off the game thread is the draws, the changed setters and the state blocks.

## Evidence

Sources:

- DXVK at `b1a1c99a` with Carafe's patches, in the patched tree `runtime/build/dxvk-src`;
- the cloud study `D:\debug_switch\carafe\cloud\results-native-d3d9\files\native-d3d9` (`REPORT.md`, `skeleton/`,
  `data/`);
- the console run `D:\debug_switch\carafe\nfsu2-exp9\logs\autorun_runtime.log`, 1887 MHz, handheld.

### What the game thread does today

- **Counts.** Line 1329, the race interval with the most draws, per frame over 600 frames:

  | Call | Per frame | Unchanged |
  |---|---|---|
  | Draws | 1509 | — |
  | Draws from memory | 254 | — |
  | Render states | 17738 | 17119 |
  | Sampler states | 14710 | 14498 |
  | Texture-stage states | 7664 | 7647 |
  | Textures | 2657 | 2177 |
  | Stream sources | 816 | — |
  | Index buffers | 814 | — |
  | Vertex shader constant sets | 2241 | — |
  | Pixel shader constant sets | 492 | — |
  | Buffer locks | 5 | — |
  | State block applies | 308 | — |

- **Time.** In the race the main thread is 98 % busy. `d3d9.dll` is 17.4 % of its x86 time, the game's code 76.5 %
  (lines 1029–1032; 15.9–18.6 % over the race intervals). At about 45 fps a frame is 22 ms, or 41.9 million cycles,
  so `d3d9.dll` costs about 3.8 ms, or 7.2 million cycles, a frame.
- **How the setters work** (`src/d3d9/d3d9_device.cpp`):
  - The unchanged path returns before the lock (`2318-2328`, Carafe `0004`).
  - A changed state takes the lock, writes `m_state` and sets dirty flags (`2331-2420`). Nothing is sent to `dxvk-cs`
    yet.
  - The work comes at the draw: `DrawIndexedPrimitive` (`3112-3173`) calls `UploadPerDrawData` and `PrepareDraw`
    (`7433-…`: hazards, uploads, samplers, textures, blend, depth and rasterizer state, shader constants,
    specialisation). It then appends to the current CS chunk, merged with the previous draw when nothing changed.
  - `DrawPrimitiveUP` (`3176-3224`) copies the vertices into an upload buffer and clears stream 0.
- **State blocks.** `Apply` (`src/d3d9/d3d9_stateblock.cpp:64-75`, template at `d3d9_stateblock.h:189-269`) replays
  every captured state:
  - render states through the public `SetRenderState`, so they are counted and early-out;
  - samplers, textures and texture stages through the internal `SetState*` functions, which skip both.
- **DXVK's CS thread** (`src/dxvk/dxvk_cs.h:15`, `dxvk_cs.cpp:117-127, 193-260`). The front end fills 16 KB chunks
  of lambdas and hands each chunk over under a mutex (`EmitCsChunk`, `d3d9_device.cpp:5953-5967`). `dxvk-cs` runs
  them against `DxvkContext`, which makes the Vulkan calls through winevulkan's crossings. In the race `dxvk-cs` uses
  35–40 % of core 3, about 40 % of it waiting in `svc 0x34` (line 1049).

### Instruction cost per call, translated by FEX (counted, not timed)

From the cloud study's `data/callmix.txt` and `data/thunk-emitted-counts.csv`, likely path, A57 setup:

| Call | DXVK today | Proxy queues it |
|---|---:|---:|
| SetRenderState, changed | 58 | 54 |
| SetRenderState, unchanged (with `0004`) | 25 | 54 without a filter, about 25 with one |
| SetSamplerState, changed / unchanged | 130 / 41 | 56 |
| SetTextureStageState, changed | 95 | 56 |
| SetTexture, changed | 242 | 64 |
| SetStreamSource | 177 | 64 |
| SetVertexShaderConstantF, 8 registers | 141 | 135 |
| DrawIndexedPrimitive with PrepareDraw | 1027 | 64 |
| DrawPrimitiveUP | about 1200 + copy (estimate from `PrepareDraw` and `DrawPrimitiveUP` rows of `dxvk-emitted-counts-all-functions.csv`) | 64 + copy |

`q_IDirect3DDevice9_SetRenderState` in `skeleton/gen/guest_gen.c:1592-1602` is a reserve and four stores. The 54
counted for it include the call and FEX's block entry.

### The skeleton as it stands

`skeleton/gen/methods.csv` and `skeleton/guest/d3d9_threaded.c`:

- **Queued:** 34 Device methods: the setters, `Clear`, `DrawPrimitive`, `DrawIndexedPrimitive`, `Begin/EndScene`.
- **Synchronous:**
  - `DrawPrimitiveUP` and `DrawIndexedPrimitiveUP`;
  - every `IDirect3DStateBlock9` method, `Capture` and `Apply` included;
  - `Create/Begin/EndStateBlock`;
  - every `Lock`/`Unlock`;
  - `Present`.
- **How a synchronous call works:** `threaded_call` submits the ring, waits for the worker to go idle, and then runs
  the call on the game thread.
- **Its two rings** of 16384 words (`include/d3d9_wire.h:6`) hand over through a critical section and two condition
  variables (`d3d9_threaded.c:9-60`).
- **No shadow state**, so it does not drop unchanged states.

So, as written, the 308 state block applies and 254 UP draws in each race frame would cost 562 drains a frame, each
waiting for the worker to empty the ring. With those, the worker never runs alongside the game thread, and the design
gains nothing.

### Arithmetic on the race frame

ARM64 instructions on the game thread per frame. Counts are from line 1329, costs from the table above:

| Part | Today | With the proxy |
|---|---:|---:|
| Draws, 1509 × 1027 / × 64 | 1.55 M | 0.10 M |
| UP draws, 254 × ~1200 / × 64 + copy | 0.30 M | 0.03–0.06 M |
| Unchanged states: 17119 RS × 25, 14498 SS × 41, 7647 TSS × ~30, 2177 tex × ~30 | 1.32 M | 0.83–1.04 M (20–25 each, filter in the proxy) |
| Changed states: 619 RS, 212 SS, 17 TSS, 480 tex | 0.18 M | 0.08 M |
| Streams and indices, 1630 × 120–177 / × 64 | 0.24 M | 0.10 M |
| Constants, 2241 VS × 141 + 492 PS × ~120 / × 135 | 0.38 M | 0.37 M |
| State block applies, 308 × 1000–3000 (states per block not measured) / mirror in the shadow, 308 × ~200 | 0.3–0.9 M | 0.06 M |
| Shaders, declarations, FVF and other setters not counted by `0005`, ~1500 × 60 | 0.1 M | 0.1 M |
| **Total** | **4.4–5.0 M** | **1.7–1.9 M** |

- **The model against the clock.** Today's 4.4–5.0 M instructions against 7.2 M measured cycles means an IPC of
  0.6–0.75, plausible for code that is 67 % loads and stores. Each vtable call from D3DX also costs an indirect
  branch through FEX's lookup in both designs, so it stays.
- **What the proxy keeps:** k = 1.7–1.9 / 4.4–5.0 = **0.34–0.43**. With unknowns, such as the states per block and
  the bytes per UP draw, the range is 0.30–0.50.
- **The speed-up** of the main thread is 1 / (1 − D (1 − k)), with D the share of `d3d9.dll`:

| D | k = 0.50 | k = 0.40 | k = 0.30 |
|---|---|---|---|
| 0.16 | 1.087 | 1.106 | 1.127 |
| 0.174 | 1.095 | 1.116 | 1.139 |
| 0.18 | 1.099 | 1.120 | 1.144 |

- **Without the game-side filter** the unchanged rows cost 41441 × 54 = 2.24 M instead of 0.83–1.04 M. k rises to
  0.6–0.7, and the gain falls to 1.05–1.07×.
- **With state block applies left synchronous** the gain is about 1.0×: every apply drains the ring.

### The worker's side

- **Work.** It runs everything the game thread no longer does: about 2.7–3.1 M instructions, or 3.6–4.5 M cycles at
  the same IPC. On top of that comes decoding, about 40 instructions a record, 9000–10000 records a frame (0.4 M).
  It needs 9–12 % of a core at today's frame rate, and 12–16 % at the rate the gain allows.
- **Room.** Cores 0 and 2 run about 10 % each in the race (line 1029: `dxvk-queue` 6.9 % and the game's `104w` 5.0 %
  on core 0; `dxvk-submit` 2.6 % and `wine_dsound_mixer` 1.3 % on core 2).
- **`dxvk-cs` on core 3** goes from 35–40 % to about 40–46 % at the higher frame rate. That is still room.

### Ring size and memory traffic

- **Words per frame.** About 9000–10000 records of 4–6 words, plus the constants' payload: 2241 × 8 registers × 4
  words. That is about 100 000–130 000 words, or 400–520 KB a frame.
- **At 50 fps** that is 20–26 MB/s written and read once. LPDDR4 gives many GB/s, so bandwidth is no concern. The
  ring should still stay in the 2 MB shared L2: a ring of 128–256 KB, consumed while it is filled.
- **The skeleton's two 64 KB rings** would hand over 6–8 times a frame, each a mutex and a wake.

## Design

**Boundary.** The COM vtable that the game calls:

- an i386 `d3d9.dll`, generated by the cloud study's `tools/gen_d3d9_thunk.py`, installed in front of DXVK's DLL,
  renamed `d3d9_dxvk.dll` (`REPORT.md` §4, "Building it in Carafe");
- it is generic for every Direct3D 9 game, with no game code involved;
- the worker is a Wine thread, so it can run DXVK's translated code. A native pthread cannot.

**On the game thread, per call:**

- **Setters with a single value:** render, sampler and texture-stage states, `SetTexture`, `SetStreamSource`,
  `SetIndices`, the shaders, the declaration and FVF. Compare with the shadow and return `D3D_OK` when equal. This is
  the same work as DXVK's `0004`, in about 10 i386 instructions with no stack frame. Otherwise write the shadow and
  append a record.
- **Constants:** copy the registers into the record. A shadow compare of 8 × 16 bytes costs as much as the copy, so
  constants are always queued.
- **Draws:** append. `DrawPrimitiveUP` and `DrawIndexedPrimitiveUP` copy their user data into the record and clear
  stream 0 in the shadow, as DXVK does (`d3d9_device.cpp:3218-3222`).
- **State blocks:** the proxy keeps its own copy of what each block captured:
  - `CreateStateBlock`, `BeginStateBlock` and `EndStateBlock` record the captured set;
  - `Capture` copies from the shadow;
  - `Apply` writes the block into the shadow and queues one record, "apply block N".

  This is what wined3d's command stream does with its client-side state block.

**Answered from the shadow, with no drain:**

- `GetRenderState`, `GetSamplerState`, `GetTextureStageState`, `GetTransform`, `GetViewport`, `GetMaterial`,
  `GetLight`, `GetClipPlane`, the `Get*ShaderConstant*` calls;
- `GetTexture`, `GetStreamSource`, `GetIndices`, `GetRenderTarget`, `GetDepthStencilSurface`: these return proxy
  objects with a reference added;
- `AddRef` and `Release`, counted in the proxy; the last `Release` is queued.

**Answered without waiting:**

- `Lock` with `D3DLOCK_DISCARD` or `NOOVERWRITE` on a dynamic buffer: the proxy hands out memory from its own staging
  ring and queues the copy at `Unlock`;
- `Present`: queued; it returns the last result the worker reported;
- `GetData` on a query: `S_FALSE` until the worker has the result, which the API allows.

**Draining calls:** the worker empties the ring, then the call runs:

- `Lock` without those flags, `GetRenderTargetData`, `GetFrontBufferData`;
- `Create*` (rare in the race, but they need DXVK's result);
- `Reset`, `TestCooperativeLevel`, and `GetData` with `D3DGETDATA_FLUSH` in a loop.

**Hand-off:**

- **One ring** of 128–256 KB, with a single producer and a single consumer. The game thread publishes its write index
  every 64 records or 16 KB, the size DXVK uses for its CS chunks, and at every `Present` and drain.
- **The worker** spins for a short time (a measured fraction of a frame) before it sleeps on a futex. A ring that
  stays busy then costs no wake at all.
- **Memory ordering.** Autorun runs FEX with TSO off (`hw/fex_options.h:47-49`), so plain x86 stores are not ordered
  on the A57. The index publication and the read must be `lock`-prefixed or `xchg` instructions. A compiler's release
  store is a plain `mov` on i386 and would be wrong here.

**x87 and window hooks.** The proxy sets the 24-bit precision control on the game thread when DXVK would (no
`D3DCREATE_FPU_PRESERVE`). The worker sets its own, since DXVK's float code now runs there. DXVK's window procedure
hook (`d3d9_window.cpp`) can call into the device from the game thread; that call drains first.

**How it differs from `dxvk-cs`.** `dxvk-cs` sits behind DXVK's front end. Everything that turns Direct3D 9 state
into DXVK state still runs on the game thread:

- validation;
- dirty tracking;
- `PrepareDraw`;
- constant uploads;
- texture reference counts.

The front end has to stay there today because the API answers synchronously: `Get*`, `Lock` pointers, queries. The
proxy moves the split up to the API itself and pays for it with the shadow state. DXVK's own chunk queue and `dxvk-cs`
stay as they are, so the pipeline gains one stage and one thread.

**Prior art.** Each of these splits the work at the API and answers the application from its own copy of the state:

- Wine's wined3d command stream (CSMT): the client-side state block, and maps with discard served from new buffer
  storage;
- Gallium Nine's `csmt`;
- Dolphin's dual-core mode: the CPU thread fills a FIFO that the GPU thread drains, with synchronisation points for
  readbacks.

Those designs are cited from what is known of them; their sources were not re-read for this note.

## What this means for the orchestrator

- **Placement.** The orchestrator places the worker on the idle core of cores 0–2 that the main thread is not on, at
  priority 59. It may move it to core 3 next to `dxvk-cs` when `[THREADS]` shows room. Without the forwarder, cores
  0–2 are the only choice.
- **Spin length.** It sets how long the worker spins before it sleeps, from the measured cost of a wake (C3). When
  the worker falls behind and the ring fills, it shortens the spin or raises the worker by one priority level.
- **Where the gain counts.** W3 is the D3D part of W1 and W2. The `Present` record is where one frame of latency
  could be taken (W2).
- **With W4.** If W4 removes D3DX's unchanged calls before they reach the device, the unchanged rows above leave the
  game thread for good. D falls to about 12–13 %, k falls to 0.15–0.25, and W3's own gain is then 1.10–1.12×. The
  gains do not add.

## Needs the console or the lab

| Measurement | How | Decision it switches |
|---|---|---|
| Synchronous calls per frame | Extend DXVK `0005`: count `Get*`, `Lock` by flags, `GetData`, `Create*`, `Present`, `StretchRect`, `GetRenderTargetData` per frame; lab for every scene, console race | More than about 10 drains a frame → staging locks and shadow answers are needed before stage 1 ships |
| States per state block apply | `0005`: count the states each `Apply` replays, by kind | The size of the state block mirror; the 0.3–0.9 M row |
| Bytes per UP draw | `0005`: sum `PrimitiveCount × stride` for `DrawPrimitiveUP` and `DrawIndexedPrimitiveUP` | The copy cost on the game thread |
| Cost of a hand-off | `[A57]`: a ring between two cores (store, `lock`-ed index, the other core reads), and a futex wake to a running thread, in ns | Spin or sleep; how often to publish (C3) |
| Proxy counters | The skeleton's `D3D9W_ENTRY_STATS` (queued, synchronous, flushes) per frame, lab first | Whether k is near 0.3 or 0.5 |
| Main thread and worker shares | `[THREADS]` and `[PROF]` before and after the proxy in the race: the main thread should drop by D(1 − k), the worker should take about D(1 − k) plus decoding | Whether the model holds; whether a ring of 128 KB keeps the worker on time |
| fps and GPU busy | `[FPS]`, `[CLOCKS]` gpu busy with the proxy on and off, same track | Ship the Experimental row or not |
