# W2. Pipelining frames: what one frame of latency buys and what stays in order

## Answer

A pipeline by frames lets stage A work on frame N+1 while stage B finishes frame N. Throughput is then set by the
slowest stage, not by the sum, and each stage boundary that holds a whole frame adds up to one stage time of input
latency. For NFSU2 under DXVK two boundaries already exist: the game thread ahead of `dxvk-cs`, and the CPU ahead of
the GPU by up to `min(3, BackBufferCount + 1)` frames. Neither is full: the main thread is 96–99 % busy and the GPU
46–73 %, so the queues are short and latency today is close to one CPU frame plus the GPU and the wait for the
display. What a further pipeline can buy depends on where it cuts:

- **At the Direct3D boundary** (W1's worker): the gain is W1's, 1.10–1.17× at level 1 and up to the GPU cap of
  1.37–2.2× at level 2. Streaming the records (the worker trails the game by a chunk, not a frame) costs about 1–2 ms
  of latency, and total latency falls, because frames get shorter. Holding a whole frame in the queue adds only
  0–5 % on average, by absorbing spikes (shader compiles, uploads), at the price of one worker-frame (4–9 ms) of
  latency. The queue should be bounded at two Presents, as PCSX2 does.
- **Inside the game's own code** (simulation of frame N+1 while the game's render traversal of frame N runs): the
  bound is 1.5–1.7× if the game's frame splits into two halves of 40–60 %, capped by the GPU. The game is not written
  for it, and the only general way is speculation on a snapshot of the game's memory. That pays only if the main
  thread writes few distinct pages per frame: with under about 300 pages at 5–10 µs a fault it costs 1.5–3 ms a
  frame and can win; with thousands it cannot. This needs measurements P3 and P4 before anything is built.

So W2 adds little on its own; its value is as W1's queue policy, and as the upper bound for a future speculative
frame split.

## Evidence

### What exists today in DXVK

Read in `runtime/build/dxvk-src` at `b1a1c99` (3.1.1).

- The game thread records closures into 16 KB chunks; `dxvk-cs` replays them (`dxvk_cs.h:15`,
  `DxvkCsThread::dispatchChunk` in `dxvk_cs.cpp:117`, no queue cap).
- `Present` (`D3D9SwapChainEx::PresentImage`, `d3d9_swapchain.cpp:840-940`) flushes, calls
  `Presenter::acquireNextImage` on the game thread, which waits until the previous present has been processed
  ("Don't acquire more than one image at a time", `dxvk_presenter.cpp:81-109`), queues the blit and present to the
  CS thread, then `SyncFrameLatency` waits for frame `id − GetActualFrameLatency()`.
- `GetActualFrameLatency` = min(device latency, default `DefaultFrameLatency` = 3 (`d3d9_device.h:213`),
  `d3d9.maxFrameLatency` if set, `BackBufferCount + 1`) (`d3d9_swapchain.cpp:1152-1159`).
- `dxvk.latencySleep` (v2.6+, https://github.com/doitsujin/dxvk/blob/master/dxvk.conf) paces the CPU to cut the
  queue when the GPU limits; it assumes input is read after `Present` returns. It does nothing for a CPU-bound game.

So the game thread can run at most one present ahead of `dxvk-cs`, and the CPU up to three frames ahead of the GPU.
D3D9Ex's own maximum frame latency is "typically 3, but can range from 1 to 20"
(https://learn.microsoft.com/en-us/windows/win32/api/d3d9/nf-d3d9-idirect3ddevice9ex-setmaximumframelatency).

### What emulators and drivers do

- PCSX2's MTGS stalls the EE thread when `s_QueuedFrameCount` reaches `VsyncQueueSize`, default 2. The source says:
  "If the MTGS is allowed to queue a lot of frames in advance, it creates input lag"
  (https://github.com/PCSX2/pcsx2/blob/master/pcsx2/MTGS.cpp, `MTGS::PostVsyncStart`).
- PPSSPP's Vulkan backend has "Buffer graphics commands" (`InflightFrames`, default 2; options "No buffer", "Up to
  1") (https://github.com/hrydgard/ppsspp/blob/master/Core/Config.cpp).
- Dolphin: "Immediately present XFB copies" "slightly reduce[s] input latency" and "does not impact performance in
  most cases" (https://dolphin-emu.org/docs/guides/performance-guide/, read via web.archive.org). "Sync GPU thread"
  bounds the CPU's lead over the GPU in emulated cycles (`SyncGpuMaxDistance` 200000), for correctness, not latency.
- NVIDIA: Low Latency Mode "Off" lets the driver queue 1–3 frames, "On" limits it to 1, "Ultra" submits just in time;
  "up to 33 %" lower latency, best when GPU-bound at 60–100 fps
  (https://www.nvidia.com/en-gb/geforce/news/gamescom-2019-game-ready-driver). Reflex: latency grows when "the CPU
  is processing frames faster than the GPU can render them causing this backup"
  (https://www.nvidia.com/en-us/geforce/news/reflex-low-latency-platform/). Both say the same thing: a queued frame
  costs latency only when the downstream stage is the limit.
- Engines written for it run three frames at once. Naughty Dog's The Last of Us Remastered on PS4: game logic of one
  frame, rendering of the previous, GPU on the one before (Christian Gyrling, "Parallelizing the Naughty Dog Engine
  Using Fibers", GDC 2015, https://www.gdcvault.com/play/1022186/parallelizing-the-naughty-dog-engine; the stage
  layout is from secondary summaries, not verified in the slides). The engine keeps separate memory per frame in
  flight, which a binary game does not have.
- No primary study measuring input latency per queued frame was found. The arithmetic: a full queued frame adds one
  frame time, 22 ms at 45 fps, 17 ms at 60.

### The latency of NFSU2 today and after a split

Assumptions (not measured): the game reads input at the start of its frame; DXVK flushes to the GPU during the frame
(`ConsiderFlush`), so the GPU overlaps the CPU in part; the display runs at 60 Hz with FIFO presentation.

| | CPU frame | after Present | GPU tail | wait for vsync, mean | input to scan-out |
|---|---|---|---|---|---|
| Today, 45 fps | 22 ms | 1–3 ms (`dxvk-cs`) | 5–10 ms | 8 ms | about 36–43 ms |
| W1 level 1, streamed, 52 fps | 19 ms | 1–3 ms | 5–9 ms | 8 ms | about 33–39 ms |
| W1 level 1, whole frame queued | 19 ms | +3–4 ms (worker frame) | 5–9 ms | 8 ms | about 36–43 ms |
| W1 level 2, streamed, 60 fps cap | 17 ms | 2–4 ms | 5–9 ms | 8 ms | about 32–38 ms |

A split that streams does not cost latency; a split that holds a full frame costs one worker-frame. Raising
`d3d9.maxFrameLatency` or the back-buffer count buys nothing while the CPU limits.

### What must stay in order

Whatever the cut, these have to be kept:

1. Device calls run in program order on one stream: the game's direct calls, the effect framework's calls and
   `Present`. The W1 proxy keeps one ring for all of them.
2. Data is captured when the call is made: constants, matrices and `DrawPrimitiveUP` vertices are copied into the
   record; locked memory is copied or renamed at `Unlock`. A pointer into game memory never crosses the queue.
3. Results the game reads come back no earlier than the work that makes them: queries return S_FALSE until ready,
   which the API allows (https://learn.microsoft.com/en-us/windows/win32/direct3d9/queries); readback locks drain.
4. Object lifetime: `Release` to zero is queued, the object is destroyed after its last queued use.
5. Device-wide events drain: `Reset`, lost devices, window-mode changes.
6. Side effects on the calling thread stay on it: the x87 control word set at `CreateDevice` without
   `D3DCREATE_FPU_PRESERVE` (`d3d9_device.cpp:96-99`, `SetupFPU`).
7. Frame pacing: the queue holds at most two Presents; beyond that the game thread waits.

For a split inside the game's code the list grows to all memory the two halves share: the simulation of frame N+1
must not change what the render traversal of frame N reads. A game that was written single-threaded gives no such
guarantee, which is why only a snapshot or speculation can make that split.

### The bound for a split inside the game

The game's frame is input, simulation (physics, AI, audio), then the render traversal (culling, D3DX effects,
Direct3D calls), then `Present` (T3 is open). Let the render traversal plus Direct3D be a share R of the main thread
and simulation 1 − R. With the two in a pipeline, throughput is 1 / max(R, 1 − R) of today's:

| R | bound |
|---|---|
| 0.5 | 2.0× |
| 0.6 | 1.67× |
| 0.7 | 1.43× |

With W1 level 2 running Direct3D and D3DX on a third stage, R shrinks by 0.38–0.43, so the simulation half becomes
the larger. All of it is capped at 1.37–2.2× by the GPU.

The closest workable approximation without the game's cooperation is copy-on-write of the game's writable memory at
`Present`: the render half of frame N reads the snapshot while the simulation of N+1 writes the live copy. Its cost is
one write fault per page written per frame. The cost of a fault through Horizon, Autorun's exception path and Wine is
not known; 5–10 µs is assumed. With 300 pages a frame that is 1.5–3 ms; with 3000, 15–30 ms, more than the gain. It
also requires knowing where the frame's halves begin and end in the game's code (T3), and every pointer into the
snapshot must be redirected, which a binary translator can do only for loads it translates. This is a research
prototype, not a stage.

## What this means for the orchestrator

- The W1 worker's ring is the pipeline. Its policy belongs to the orchestrator: stream records in chunks
  (DXVK's 16 KB is a good size); wake the worker per chunk, not per frame; stall the game thread when two Presents are
  queued.
- Leave `d3d9.maxFrameLatency` and the back-buffer count alone while the CPU limits; set `maxFrameLatency = 1` only
  when the GPU becomes the limit (M5 of W1), to keep latency down then.
- A frame-time spike on the worker (shader compiles, uploads) is the one case where a deeper queue helps. The
  orchestrator can raise the bound from one chunk to one frame when the worker's per-frame time varies by more than a
  threshold (P2), and lower it again.
- The speculative frame split stays a measurement until P3 and P4 say whether it can pay.

## Needs the console or the lab

| Name | How | Decision it switches |
|---|---|---|
| P1 frame timeline | per frame: when the game calls `IDirectInputDevice8::GetDeviceState` (or reads the keyboard/pad), when it calls `Present`, when `dxvk-cs` runs the present, when the GPU fence signals; in the profiler (console) | today's latency; the bound for the queue depth; whether input is read at the start of the frame |
| P2 worker spikes | histogram of `dxvk-cs` (and later the W1 worker) busy time per frame (console) | stream in chunks, or allow a full frame in the queue |
| P3 pages written per frame | from the T4 trace on a PC: distinct 4 KB pages the main thread writes between two Presents, split by heap, stack and image | whether a snapshot split of the game's frame can pay (under about 300 pages) |
| P4 write-fault cost | `[A57]`: protect a page, write, handle the fault in the runtime, unprotect, resume; µs per fault (console; the lab gives the order on Eden) | the same |
| P5 Present wait | wait time on the main thread in `acquireNextImage` and `SyncFrameLatency` (W1 M4) | whether the CPU ever waits for the GPU today |
