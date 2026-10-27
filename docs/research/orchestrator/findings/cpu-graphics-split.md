# W1. Splitting CPU and graphics work, as Dolphin and RPCS3 do

## Answer

Console emulators split one guest stream of work at the boundary where the guest hands commands to the GPU: a
producer thread writes a FIFO, a consumer thread drains it, and the producer waits only at a short list of sync
points (reads of GPU-produced data). The gain comes from two things: the consumer's work leaves the producer's core,
and the sync points are made rare by answering from a CPU-side state copy, renaming buffers and deferring readbacks.
Under DXVK half of this split already exists: `dxvk-cs` records Vulkan on another core. What stays on the game
thread is DXVK's whole Direct3D 9 front end (validation, state tracking, dirty tracking, constant packing,
`PrepareDraw`, closure recording) and, above it, the D3DX effect framework that the game links into its `.exe`.
The split can be moved up one or two levels:

- **Level 1, at the `IDirect3DDevice9` interface** (the cloud study's i386 proxy "C", with unchanged states dropped
  on the game side, W3): the game thread keeps about 0.2–0.3 of today's 16–18 % Direct3D share. Gain
  **1.10–1.17×** on the main thread, if round trips per frame stay below about 50 (condition measured by M1).
- **Level 2, at the `ID3DXEffect` interface as well** (the effect calls queued, D3DX's state re-application and DXVK's
  front end both run on the worker): the game thread loses 38–43 % of its time and keeps 5–15 % of that. Gain
  **1.45–1.65×** on the main thread; with the larger D3DX share the 2026-10 profile hints at (about half of the
  game's 75 %), up to about 2×. It needs W4 (finding D3DX's effect vtable in the `.exe`) and effect-level shadow state.
- **The frame rate is capped by the GPU**: it was 46–73 % busy at today's rate, so the CPU split alone can raise the
  rate by at most 1/0.73 = 1.37× in the heaviest scenes and 1/0.46 = 2.2× in the lightest. Level 2 reaches that cap
  in heavy scenes; level 1 does not.

So the realistic range for NFSU2 is 1.10–1.17× (level 1) and 1.35–1.6× in fps (level 2, GPU-capped), with the
worker using 0.6–0.7 of another core at the higher rate.

## Evidence

### How the emulators split

Dolphin (GameCube/Wii), dual-core mode. Source: `Source/Core/VideoCommon/Fifo.cpp`
(https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/VideoCommon/Fifo.cpp).

- The CPU thread writes the emulated gather pipe into the CP FIFO in emulated memory. The GPU thread
  (`FifoManager::RunGpuLoop`) reads it in 32-byte blocks, decodes with `OpcodeDecoder::RunFifo`, and moves
  `CPReadPointer` and `CPReadWriteDistance` atomically. CPU-thread requests go through `AsyncRequests`: `PushEvent`
  (fire and forget: XFB swap, EFB pokes) or `PushBlockingEvent` (a future the CPU thread waits on)
  (`VideoCommon/AsyncRequests.h`).
- Sync points where the CPU thread blocks: EFB peeks (`EFBInterface.cpp`, `PeekColorInternal`, `PeekDepthInternal`),
  the bounding box (`Video_GetBoundingBox` → `SyncGPU(SyncGPUReason::BBox)`), performance queries
  (`Video_GetQueryResult`), save states, and reads of CP status registers (`SyncGPUForRegisterAccess` in
  `CommandProcessor.cpp`, added by PR #7214, https://github.com/dolphin-emu/dolphin/pull/7214, to stop games that
  poll the GPU seeing it stalled; cost 5–7 % in light scenes).
- "Sync GPU thread" (`SyncGPU`, defaults in `Core/Config/MainSettings.cpp`: MaxDistance 200000, MinDistance −200000
  emulated cycles, overclock 1.0) bounds how far the CPU may run ahead of the GPU in emulated cycles; the GPU thread
  runs in slots of 1000 cycles. It trades speed for stability.
- "Deterministic dual core" (comex, 2014) reads the FIFO on the CPU thread and answers GPU→CPU queries with fake
  completion; the GPU thread only runs the opcode decoder on a preprocessed buffer. The progress report calls it
  "slightly more taxing than standard Dualcore. But it's still much faster than single core"
  (https://dolphin-emu.org/blog/2014/09/30/dolphin-progress-report-september-2014/).
- Determinism problems: dual core is "the source of most crashes"; games that race the CPU against the GPU break
  when host timing changes. Dual core is off by default on desktop since 2506-334 and stays on for Android, where
  the cores are weaker (https://dolphin-emu.org/blog/2025/09/16/dolphin-progress-report-release-2509/; read via
  web.archive.org; the speed charts are images, numbers not verified).
- Deferring a round trip: "Defer EFB copies to RAM" (PR #7539, https://github.com/dolphin-emu/dolphin/pull/7539)
  queues EFB copies until the guest's own sync events (DrawDone, PE tokens). i7-6700K, GTX 1070, Vulkan: Xenoblade
  62 → 156 fps, Wind Waker 105 → 130, Silent Hill: Shattered Memories 49 → 55
  (https://dolphin-emu.org/blog/2018/12/04/dolphin-progress-report-november-2018/).

RPCS3 (PS3). Source: `rpcs3/Emu/RSX/RSXFIFO.cpp`, `RSXThread.cpp`, `RSXZCULL.cpp`, `RSXOffload.cpp`
(https://github.com/RPCS3/rpcs3/tree/master/rpcs3/Emu/RSX).

- The PPU writes RSX commands to memory and moves the `put` register; `rsx::FIFO::FIFO_control` reads `put`/`get` in
  shared VM memory in 128-byte chunks and spins when they meet. The RSX thread runs ahead; the PPU waits only on
  semaphores, labels and reports.
- ZCULL reports (occlusion results) are queued in `m_pending_writes` and retired later; "Relaxed ZCULL sync" stops
  hard-waiting for them (PR #6940, https://github.com/RPCS3/rpcs3/pull/6940). Tooltip: it "can greatly improve
  performance in some games or completely break others" (`rpcs3qt/tooltips.h`).
- "Multithreaded RSX" (`rsx::dma_manager`) moves large CPU→GPU copies to a worker; 20–50 % "depending on the
  system", off by default because quad cores are too weak and the worker flickers when descheduled
  (https://rpcs3.net/blog/2019/07/31/progress-report-june-2019/, PR #6112).
- "Write/Read color buffers" are the RPCS3 form of readback round trips; both cost speed and stutter.

PCSX2 (PS2), MTGS. Source: `pcsx2/MTGS.cpp` (https://github.com/PCSX2/pcsx2/blob/master/pcsx2/MTGS.cpp).

- The EE thread fills a ring (`PrepDataPacket`, `SendDataPacket`); the GS thread drains it. The EE stalls when
  `s_QueuedFrameCount` reaches `VsyncQueueSize` (default 2): "If the MTGS is allowed to queue a lot of frames in
  advance, it creates input lag".
- The design note: "99% of the communication … happens in the direction to the GS". FINISH/SIGNAL register syncs do
  not cross threads: "the EE thread peeks at all the packets ahead of time and handles it in its own routines". GS
  downloads drain the ring (`WaitGS`), unless the user picks unsynchronized downloads. Gain: "Usually frame rates go up
  or even surpass 2x" (PCSX2 wiki, "Graphics Synthesizer, GPUs and Dual Cores", read via web.archive.org).

Others, same pattern: Xenia's `CommandProcessor::WorkerThreadMain` drains the guest ring by its write pointer
(https://github.com/xenia-project/xenia/blob/master/src/xenia/gpu/command_processor.cc); xemu's `pfifo_thread`
(`hw/xbox/nv2a/pfifo.c`); yuzu/Eden's `VideoCommon::GPUThread::ThreadManager` (`src/video_core/gpu_thread.cpp`),
where async mode skips `FlushRegion`; moving query downloads and fence checks off the guest thread gave "up to 87 %
… (50 % on average)" (https://yuzu-mirror.github.io/entry/yuzu-progress-report-apr-2023/).

Wine's wined3d CSMT is the closest prior art, because it splits Direct3D 9 itself.

- All GL calls, "including resource creation and buffer maps", go to one thread. "A lot of games see 50%-100%
  performance improvements"; GPU-limited games gain nothing (Stefan Dösinger, wine-devel, 2013-09,
  https://www.winehq.org/pipermail/wine-devel/2013-September/101106.html). On by default since Wine 3.3.
- What stays on the application thread: the d3d9 state copy (`d3d9_device_GetRenderState` reads
  `device->stateblock_state`, `dlls/d3d9/device.c`); DISCARD and NOOVERWRITE maps, answered with an upload buffer
  (`wined3d_device_context_emit_map` → `wined3d_cs_map_upload_bo`, `dlls/wined3d/cs.c`). Any other map waits for the
  resource and makes a full round trip. Queries return S_FALSE until the CS has caught up and never block
  (`wined3d_query_get_data`, `dlls/wined3d/query.c`).

The gains differ for a reason. Wine CSMT and the emulators move most of the graphics work off the producer (the GL
driver, or the whole emulated GPU). Under DXVK the Vulkan side has already left the game thread, so only the front
end is left to move; this is why level 1 is worth 1.1–1.17×, not 1.5–2×.

### What stays on the game thread under DXVK today

Read in `runtime/build/dxvk-src` at `b1a1c99` (3.1.1).

- Every `IDirect3DDevice9` method runs on the calling thread under `D3D9DeviceLock`. State lives in `m_state`;
  setters compare, store and set `m_dirty` flags; Carafe's `0004` returns before the lock when nothing changes.
- Get* calls are answered from `m_state` with no CS sync (`D3D9DeviceEx::GetRenderState`, `d3d9_device.cpp:2688`).
  DXVK accepts `D3DCREATE_PUREDEVICE` but answers Get* anyway (`d3d9_interface.cpp:380-392`).
- `PrepareDraw` (`d3d9_device.cpp:7433`) runs per draw on the game thread: managed buffer and texture uploads,
  fixed-function shader updates, `Bind*` for each dirty group, `UndirtySamplers`, `UndirtyTextures`, and
  `UpdateShaderConstants`. Constants are copied by the game thread into a mapped buffer (`D3D9ConstantBuffer::Alloc`,
  `d3d9_constant_buffer.cpp:30-70`), so the game thread writes host-visible memory (C7).
- Every result is captured into a closure pushed into a 16 KB chunk (`EmitCs`, `d3d9_device.h:1267`;
  `DxvkCsChunkSize`, `dxvk_cs.h:15`). Full chunks go to `DxvkCsThread::dispatchChunk`, which has no queue cap; the
  game thread is held back only by frame latency and `ThrottleAllocation`.
- `dxvk-cs` runs `DxvkContext`: barriers, descriptors, pipeline lookup, and each `vkCmd*` through FEX's unix-call
  bridge (158–175 ARM64 instructions a crossing, cloud study `native-d3d9`).

Where the game thread waits for `dxvk-cs` (`SynchronizeCsThread` callers):

| Call | When it waits |
|---|---|
| `Reset`, device creation and destruction | always (`SynchronizeAll`) |
| `LockBuffer` | not for DISCARD (a new slice: `DiscardMapSlice` + `invalidateBuffer`), not for NOOVERWRITE, not when no readback is pending; otherwise `WaitForResource` (`d3d9_device.cpp:5518-5660`) |
| `LockImage` | only when the image needs a readback, e.g. after `GetRenderTargetData` (`:5017-5210`) |
| `UpdateTextureFromBuffer`, `UploadPerDrawData` with a readback buffer | `WaitForResource` |
| `EnsureSamplerLimit` | only when the sampler pool is nearly full (`:7620-7670`) |
| `GetRenderTargetData` | does not wait; it queues the copy and marks the target for readback |
| `IDirect3DQuery9::GetData` | never waits; S_FALSE until ready, a flush hint with `D3DGETDATA_FLUSH` (`d3d9_query.cpp:129-160`) |
| `Present` | `acquireNextImage` waits for the previous present to be processed (`dxvk_presenter.cpp:81-109`); `SyncFrameLatency` waits for frame `id − min(3, BackBufferCount + 1)` (`d3d9_swapchain.cpp:1147-1159`) |

`DXVK_HUD=cs` shows chunks, syncs and CS load; the counters are `CsSyncCount` and `CsSyncTicks` (`dxvk_cs.cpp:187`).

### D3D9 calls that need an answer, and how each is handled when the front end moves

The Microsoft documents: Lock flags (https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dlock); queries,
where S_FALSE is a legal answer and the recommended practice is to treat a pending result as "visible"
(https://learn.microsoft.com/en-us/windows/win32/direct3d9/queries); `D3DCREATE_PUREDEVICE` drops Get* support for
state-block state (https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dcreate).

| Call | Answer needs | Way around (who does it) |
|---|---|---|
| Get* of state, `GetTransform`, `GetTexture` | the latest value set | a shadow copy on the game side (DXVK, Wine, PCSX2's packet peek) |
| `Lock` DISCARD | a pointer the GPU is not reading | rename: hand out fresh staging memory, queue a copy at `Unlock` (wined3d upload BO, DXVK's own `DiscardMapSlice`) |
| `Lock` NOOVERWRITE | the current mapping | same pointer, no wait (DXVK, wined3d) |
| `Lock` with no flag, dynamic or managed | the contents after all queued writes | for write-only use: rename and copy at `Unlock`; for reads: drain |
| `LockRect` of a readback surface, `GetRenderTargetData` | GPU-produced pixels | drain; or deferred like Dolphin's EFB copies, if the game reads the data a frame later |
| `GetData` | GPU result | return S_FALSE until the worker has it (legal, as RPCS3 relaxed ZCULL and yuzu's async queries) |
| `Create*` | a new object | a proxy object answered at once, created on the worker in order (the cloud study's ids) |
| `AddRef`/`Release` counts | the count | kept by the proxy; destruction queued |
| `Present` | nothing | queued; the proxy bounds the queue by frames, like PCSX2's `VsyncQueueSize` |
| `Reset`, `TestCooperativeLevel`, window messages | device state | drain; rare |
| `CreateDevice` without `D3DCREATE_FPU_PRESERVE` | x87 control word on the game thread | set by the proxy on the calling thread (cloud study, section 4) |

### The numbers for this game

Inputs (`../game-speed/REPORT.md`, console runs of October 2026, NFSU2 at 1887 MHz): the main thread is 96–99 %
busy; game code 77 %, of it D3DX effects 22–25 % of the main thread; `d3d9.dll` 16–18 %; GPU 46–73 % busy;
per frame up to 1500 draws, 17700 render, 14700 sampler and 7600 texture-stage states, 96.5–99.8 % unchanged, 2700
texture binds (82 % unchanged), 2200 constant sets, 300 state-block applies.

At 45 fps a frame is 41.9 M cycles; 17 % of it is 7.1 M cycles of DXVK front end. Per-call ARM64 counts from the
cloud study (today → queued): a draw ≥1027 → 64, `SetRenderState` 58 (25 unchanged after `0004`) → 54, constants
141 → 135.

Level 1, per frame, counted (estimated, instruction counts stand in for time):

- Today: draws 1.54 M, unchanged states about 39000 × 25 = 0.97 M, constants 0.31 M, textures about 0.5 M, state
  blocks and changed states about 0.7 M; total about 4 M ARM64 instructions. That matches 7.1 M cycles at an IPC of
  about 0.6.
- Queued, unchanged states dropped against a game-side shadow at about 12 instructions: draws 0.10 M, unchanged
  states 0.47 M, changed states and textures 0.1 M, constants 0.30 M, state blocks 0.02 M; total about 1 M.
- So k ≈ 0.25 (the cloud study's call mixes give 0.22–0.45 without the early drop). Speedup
  1 / (1 − D (1 − k)): D = 0.16, k = 0.3 → 1.13; D = 0.18, k = 0.2 → 1.17.

Level 2 (effects queued too):

- Removed from the game thread: E + D = 0.38–0.43. What the game thread keeps: the queued effect calls. Their count
  is not measured; at 5–15 effect calls per draw (`SetMatrix`, `SetTexture`, `BeginPass`, `CommitChanges`, `EndPass`)
  and about 60 instructions each it is 0.45–1.35 M instructions a frame against about 10 M today, k = 0.05–0.15.
- Speedup 1 / (1 − 0.38 × 0.85) = 1.48 to 1 / (1 − 0.43 × 0.95) = 1.69. With E at about 37 % (the 2026-10 note
  "about half of the game's 75 %"), up to 2.0.
- The worker then carries 0.4–0.45 of today's frame plus the queue's decode. At 1.5× today's rate that is
  0.6–0.7 of a core; `dxvk-cs` rises from 0.35–0.40 to 0.55–0.60.

The GPU cap: busy 46–73 % at today's rate gives at most 1.37–2.2× more frames before the GPU is full.

Sync points per frame decide whether any of this holds. A drain costs a wake-up and the worker's backlog. If the
worker is faster than the game thread the backlog is short and a drain costs the hand-off time (C3; tens of
microseconds is plausible, not measured). At 50 drains a frame and 20 µs each that is 1 ms of a 22 ms frame,
4.5 %; at 500 drains the split is lost. Wine's experience is the same: non-DISCARD maps are its expensive case.

## What this means for the orchestrator

A component "graphics front end" with two levels, both in the game process, both i386 code under FEX, both
generated from method lists. Neither needs native code.

1. **Level 1, `d3d9` proxy** (layer: a DLL installed by `runtime/dxvk/use-dxvk.py` as `d3d9.dll`, DXVK as
   `d3d9_dxvk.dll`; the cloud study's `skeleton/guest/d3d9_threaded.c` is the start).
   - Game side: a shadow of all state-block state; unchanged setters return at once; changed ones and draws are
     encoded into a ring of records with inline payloads (matrices, constants, UP vertex data).
   - Lock: DISCARD and NOOVERWRITE answered from the proxy's own staging, a copy record queued at `Unlock`; other
     write locks renamed the same way when the resource is write-only; reads drain.
   - Queries: S_FALSE until the worker has issued and seen the result; the proxy remembers the last result per
     query.
   - Worker: one i386 thread on core 3 (or core 2 without the forwarder), at a priority just below the game thread,
     replaying records into DXVK. `dxvk-cs` stays as it is.
   - Ring bounded by frames (two Presents, as PCSX2's queue), so input latency does not grow (W2).
2. **Level 2, `ID3DXEffect` proxy** (layer: FEX or the runtime patches the vtable of effect objects the game's linked
   D3DX creates; finding it is W4).
   - Effect calls are encoded into the same ring as device calls, so order between the game's direct device calls
     and effect passes is kept.
   - Answers the effect API needs (pass count from `Begin`, handles from `GetParameterByName`, descriptions) come
     from a cache filled on first use; they are static per effect.
   - The worker runs D3DX's `BeginPass`/`CommitChanges` and DXVK's front end. W3's early drop then runs on the worker,
     where it still saves the worker's time.
3. **The orchestrator's part**: choose the level per game from counters (M1, M2); place the worker by the core map
   (C2); turn the proxy off when drains per frame exceed a threshold; report `[D3D9 front end]` lines (queued, dropped,
   drains, worker busy).

Order: level 1 first (weeks, the cloud study's skeleton exists), because its counters measure level 2's conditions.

## Needs the console or the lab

| Name | How | Decision it switches |
|---|---|---|
| M1 `[D3D9 sync]` | extend DXVK `0005`: per frame, Lock by kind (buffer/texture) × flags (DISCARD, NOOVERWRITE, none, READONLY) × pool; `GetData` calls and S_FALSE returns, with and without FLUSH; `GetRenderTargetData`, `GetFrontBufferData`; Get* calls; Create*/Release; `CsSyncCount`/`CsSyncTicks`; device behaviour flags. Lab is enough: the counts do not depend on the console | below about 50 drains a frame: level 1 with drains; above: renaming and deferred readback per call kind first; hundreds of reads: level 1 is not worth it |
| M2 `[D3DX effect calls]` | count calls per method of the game's effect vtable per frame (lab, a FEX block profile on the vtable targets, or a hook once W4 finds it) | k for level 2; whether level 2 is worth W4's work |
| M3 hand-off cost | `[A57]`: one thread writes a record and bumps an index, another polls or sleeps on a futex; time from write to read, and from wake to run, across cores 0–3 (C3) | spin or sleep in the worker; the ring's batch size; the drain threshold |
| M4 Present wait | profiler wait states on the main thread inside `Presenter::acquireNextImage` and `SyncFrameLatency` (console) | whether part of today's 96–99 % is waiting, not work |
| M5 GPU busy after level 1 | `[CLOCKS]` GPU busy share and frame time with the proxy on (console, with the stage's Experimental row) | stop at level 1 if the GPU is above 90 % busy |
| M6 worker core | `[THREADS]` with the proxy: worker busy, `dxvk-cs` busy, the game thread's drop (console) | the worker's core and priority; whether core 3 holds both workers |
