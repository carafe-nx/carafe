# GPU submission, CPU–GPU synchronisation, and what "gpu busy" means

## Answer

A Direct3D 9 call travels through four threads before the GPU sees it: the game thread records it into DXVK's
16 KB command chunks; `dxvk-cs` (core 3) turns chunks into Vulkan commands, which NVK writes as push buffers; at a
flush the command list goes to `dxvk-submit` (core 2), whose `vkQueueSubmit` ends in one `SUBMIT_GPFIFO` ioctl to
nvservices; `dxvk-queue` (core 0) waits on the fence and frees the resources. DXVK flushes when the first render target
changes, at `EndScene`, at Present, and on resource waits, so a frame is several submissions; the count is not logged.
The game thread can block on the GPU only at a short list of places (a lock of a busy buffer without `DISCARD` or
`NOOVERWRITE`, readbacks, queries waited on, more than 30 MB of discarded or staging memory in flight, the frame
latency limit at Present), and in the race it does not: it spends 0.8–1.4 % of its samples in system calls. "gpu busy"
is the GPU's own PMU load, the share of time the graphics and copy engines were not idle, read in tenths of a percent
every 100 ms by the overlay and averaged per 10-second report. It is not 99 % because the GPU is starved: it does a
frame's work in 9–16 ms and then waits for the next submission, while the main thread needs 18–28 ms to produce a
frame. A second, independent measure agrees: `dxvk-queue` waits on a GPU fence 41–64 % of the time, 2–9 points below
`gpu busy` in every report, and waits for work the rest. The GPU's own ceiling at 614 MHz GPU and 1331 MHz memory is
fps ÷ busy = 61–107 fps, median 83: the GPU would be fed fully only when the main thread is 1.4–1.9× faster. Up to
there nothing on the GPU side needs changing; `dxvk-cs` on core 3 runs out at about 80–100 fps, near the same point.

## Evidence

Trees as in [shared-memory-attributes.md](shared-memory-attributes.md): DXVK `b1a1c99` in
`carafe/runtime/build/dxvk-src`, the console's Mesa `danfromtico/mesa-switch@d4a00ea0ab` (read from GitHub), libnx
`146c3d14`, Autorun `636913ca`, Eden `58c1e20`. Logs: `D:\debug_switch\carafe\nfsu2-exp9\logs`.

### The path of a submission

- Game thread: every D3D9 call that changes state or draws emits a lambda into the current CS chunk
  (`src/dxvk/dxvk_cs.h:15`, 16384 bytes); full chunks go to `dxvk-cs` with no queue limit
  (`src/dxvk/dxvk_cs.cpp:117-131`).
- `dxvk-cs` runs chunks (`dxvk_cs.cpp:192-262`) into `DxvkContext`, whose `vkCmd*` calls cross from the translated
  `winevulkan.dll` into native NVK. It sits on core 3 at priority 63 (`logs/SPEED2.log`, `[CORES] 44 dxvk-cs: core 3,
  priority 63`). Binding model "Legacy" (descriptor sets; `logs/SPEED2.log`, `Binding model: Legacy`).
- Flush: `ExecuteFlush` signals DXVK's fences and calls `flushCommandList` on the CS thread
  (`src/d3d9/d3d9_device.cpp:6199-6240`); `DxvkSubmissionQueue::submit` queues it, blocking only if 32 command lists
  are queued or in flight (`src/dxvk/dxvk_queue.cpp:47-64`, `src/dxvk/dxvk_limits.h:17`).
- `dxvk-submit` calls `cmdList->submit` or `presentImage` (`dxvk_queue.cpp:131-225`). In NVK this becomes
  `nvkmd_switch_ctx_exec`/`signal` (`mesa-switch:src/nouveau/vulkan/nvkmd/switch/nvkmd_switch_dev.c:361-498`), then
  `nouveau_horizon_channel_submit_locked`: GPFIFO entries, a fence, one `nvGpuChannelKickoff`
  (`src/nouveau/horizon/nouveau_horizon_channel.c:1175-1377`, kickoff 1009-1036). NVK has one graphics queue
  (`nvk_physical_device.c:1718-1721`). Submissions in flight are capped at 512 per channel
  (`nouveau_horizon_private.h:30`), far above DXVK's 32.
- `dxvk-queue` waits with `vkWaitSemaphores` on DXVK's timeline semaphores (`dxvk_queue.cpp:228-308`); NVK emulates
  timelines over its fence type (`nvkmd/switch/nvkmd_switch_pdev.c:493-498`) and waits with libnx's `nvFenceWait`: an
  `EventWaitAsync` ioctl to `nvhost-ctrl`, then `eventWait`, which is `svcWaitSynchronization`
  (`libnx:nx/source/nvidia/fence.c:90-117`), in 50 ms slices (`nouveau_horizon_channel.c:18, 57-104`). The time it
  spends with nothing to wait for is DXVK's `GpuIdleTicks` (`dxvk_queue.cpp:236-245`).
- Present: `presentInterval = 1` from Autorun's `dxvk.conf` (`logs/SPEED2.log:329-333`), FIFO, 3 images
  (`logs/SPEED2.log:692-694`). The D3D9 frame latency is min(3, BackBufferCount + 1)
  (`src/d3d9/d3d9_swapchain.cpp:1147-1159`, `src/d3d9/d3d9_device.h:213`), waited for at the end of each Present
  (`d3d9_swapchain.cpp:930`).

### When DXVK submits

- The flush tracker allows every implicit flush type on NVK (`GetMaxFlushType`, `d3d9_device.cpp:9203-9210`; NVK is not
  a tiler, `src/dxvk/dxvk_device.cpp:723-739`). Strong hints flush after 3 chunks, weak hints after 6, or at once if
  fewer than 2 submissions are pending (`src/util/util_flush.cpp:13-83`).
- Strong hints: first render target changed (`d3d9_device.cpp:1714-1722`), `EndScene` (`:1873`), event queries
  (`:7910-7924`). Weak hints: other render targets, depth-stencil surface, texture updates, buffer flushes (`:1131,
  1813, 5500, 5698`). Present is an explicit flush.
- So a frame is about one submission per render-target change plus Present. NFSU2 changes targets several times a
  frame (reflection, mirror, shadow, post passes); an estimate of 4–10 submissions a frame is not verified. DXVK counts
  them (`QueueSubmitCount`, `src/dxvk/dxvk_stats.h:13-41`) but nothing logs it.
- `dxvk-submit` uses 2.1–3.1 % of a core in the race and `dxvk-queue` 4.5–8.3 % (`[THREADS]`), so submitting costs
  0.5–0.7 ms of CPU a frame across both, off the main thread.

### Where the game thread can wait for the GPU

| Call | Wait | Source |
|---|---|---|
| `Lock` of a buffer still in use, without `DISCARD`/`NOOVERWRITE` | CS sync, flush, wait for the resource | `d3d9_device.cpp:5518-5655`, `WaitForResource` 4948-4982 |
| `Lock` with `DISCARD` when more than 30 MB of staging and discarded memory is in flight (10 MB per submission on a 32-bit host) | wait on the staging fence | `ThrottleAllocation`, `:4898-4923` |
| `LockRect` of a `DEFAULT`-pool or render-target surface, `GetRenderTargetData` | copy to a buffer, then `WaitForResource` | `:5150-5205` |
| `GetData` on a query | CS sync, flush hints | `:7910-7924`, `D3D9Query` |
| CS-thread syncs for reads of device state | waits for `dxvk-cs`, not the GPU | `SynchronizeCsThread`, `:5980-5989` |
| Present | waits until frame N − latency has been presented | `d3d9_swapchain.cpp:930, 1147-1150` |

In the race the main thread (`4w`) is in system calls 0.8–1.4 % of its samples (most of it `svc 0x1d`, signalling a
condition variable), and up to 10.7 % only in the scene changes (`[PROF] 4w samples … svc=`). It does not wait on the
GPU.

### What "gpu busy" is

- The overlay opens `/dev/nvhost-ctrl-gpu` and issues ioctl `0x80044715` every 100 ms, sums the 32-bit result and
  prints sum ÷ count ÷ 10 as a percentage (`carafe/runtime/overlay/carafe_diag.c:252-305, 355-372`).
- `0x80044715` decodes as read, 4 bytes, type `'G'`, number `0x15`: `NVGPU_GPU_IOCTL_PMU_GET_GPU_LOAD`, "struct {
  __out u32 pmu_gpu_load; }" (Switchbrew, NV services). Eden names it and does not implement it
  (`eden:src/core/hle/service/nvdrv/devices/nvhost_ctrl_gpu.cpp:28-80`), so the lab never shows it.
- Units and meaning come from NVIDIA's Linux driver, of which nvservices is a port: the PMU's perfmon counts "GR && CE2
  busy cycles" over a 16.7 ms sampling period (`gk20a/pmu_gk20a.c`, `pwr_pmu_idle_mask_gr_enabled_f() |
  pwr_pmu_idle_mask_ce_2_enabled_f()`, `perfmon_cmd_init_set_samp_period_us(..., 16700)`), and the normalised load is
  in tenths of a percent (`gk20a/gk20a_scale.c`, `busy_time * dt / 1000`; android.googlesource.com
  `kernel/tegra.git`, `android-tegra-3.10`). That Horizon's nvservices keeps the same counter, period and unit is not
  verified; the overlay's division by 10 and the plausible 13–73 % range agree with it.
- So each sample is the busy share of the graphics and copy engines over the last PMU window, for the whole GPU (every
  process, the overlay included). About 100 samples a report make the mean good to a few points. "Busy" is engine
  activity, not shader utilisation: a GPU stalled on memory counts as busy.

### Why it is not 99 % and what would feed it

From every race report (`[FPS]`, `[CLOCKS]`, `[THREADS]`, `[PROF] 40w svc:` and `44w`, `logs/autorun_runtime.log:1029-1987`):

| fps | gpu busy | `dxvk-queue` fence wait | GPU ms a frame | ceiling fps ÷ busy |
|---|---|---|---|---|
| 35.4–35.9 | 46–50 % | 41–45 % | 13.0–14.1 | 71–77 |
| 38.7–40.6 | 49–67 % | 44–61 % | 12.7–16.5 | 61–79 |
| 44.0–48.7 | 52–61 % | 48–56 % | 10.9–12.8 | 78–92 |
| 51.1–54.6 | 51–73 % | 47–64 % | 9.4–13.4 | 75–107 |

- `dxvk-queue`'s `svc 0x18` (`WaitSynchronization`, the fence wait) is 2.4–8.8 points below `gpu busy` in all 18
  reports, mean 5.1; its `svc 0x34` (`WaitForAddress`, nothing queued) is 26–48 %. With nothing queued the GPU has
  nothing to do: idle time is starvation, not a stall on a sync point.
- The main thread is 88–99 % busy in the same reports; `dxvk-cs` 27–40 % busy and waiting for work (`svc 0x34`) 31–57 %
  of its samples. The chain is limited at its head.
- Ceiling: if the GPU's work per frame stays as it is, fps ÷ busy is the GPU-bound rate: 61–107, median 83. The GPU
  runs at 614 MHz here, above the stock handheld clocks; with the GPU clock lower the ceiling falls with it, by up to
  the clock ratio if the work is clock-bound and less if it is memory-bound.
- Feeding it fully needs the main thread 1.4–1.9× faster. With a frame latency of 2–3 and 3 swapchain images the CPU can
  then run ahead enough to keep the GPU queued; no DXVK change is needed for that.
- `dxvk-cs`: 0.37 core ÷ 47 fps = 7.9 ms of CPU a frame. Its samples outside system calls (46–69 %) exceed its CPU
  time (27–40 %) by 17–32 points in every report, mean 22: it is runnable but not running that long, which fits core 3
  being shared with system threads (see [core-3.md](core-3.md); inference). If it keeps 78 % of core 3 it saturates
  near 100 fps; if the gap grows with frame rate, near 80.

## What this means for the orchestrator

- The closed loop can classify each second from data it already has: main thread ≥ 95 % and `gpu busy` well below
  90 % means CPU-bound, with fps ÷ busy as the GPU's headroom. That headroom says how much a CPU-side mechanism can win
  before the GPU becomes the limit: for NFSU2 at these clocks about 1.75×. Above it, CPU work no longer buys frames.
- `gpu busy` and `dxvk-queue`'s fence-wait share are two measures of the same thing; the second needs no ioctl and
  works in the lab, where Eden lacks the PMU ioctl.
- No synchronisation point blocks the game thread in this game. For other games the sync points in the table above
  are what the orchestrator should count (`GpuSyncCount`, `CsSyncCount` and their ticks are already in DXVK); a game that
  spins on `Lock` or `GetData` needs the flush policy or a per-game profile, not more cores.
- Submission granularity is fine while the main thread is the limit. It matters when the GPU becomes the limit or when
  frame pipelining (W2) changes when commands reach the GPU; then submissions per frame and `GpuIdleTicks` decide
  whether to flush earlier.
- `dxvk-cs` is the next bottleneck after the main thread, at roughly the same rate as the GPU ceiling. Its runnable
  time on core 3 is the number to watch.

## Needs the console

| Measurement | How, from user mode | Decision it switches |
|---|---|---|
| DXVK counters per frame in `[D3D9 calls]`: `QueueSubmitCount`, `GpuIdleTicks`, `GpuSyncCount`/`Ticks`, `CsSyncCount`/`Ticks`, `CsIdleTicks`, `CsChunkCount` | DXVK patch `0005` reads `m_dxvkDevice->getStatCounters()` at its 600-frame report and prints the deltas | submissions per frame; whether the game thread ever waits on the GPU or `dxvk-cs`; flush policy for pipelining (W2) |
| `gpu busy` at a finer grain: a histogram of the 100 ms samples per report, plus the min and max | `carafe_diag.c` keeps buckets next to its sum | whether the GPU is evenly under-fed or saturated in bursts (bursts would make fps ÷ busy optimistic) |
| Calibration of `gpu busy`: a scene that is GPU-bound (menu at 60 fps with VSync shows 36–41 %; a GPU-bound scene should read above 90 %) | existing sampler, any GPU-heavy game or a stress pass in the lab build | whether fps ÷ busy can be trusted as the GPU ceiling |
| NVK channel statistics: submissions, kickoff CPU time, in-flight peaks | `NVK_SWITCH_PERF_LOG=1` set by the overlay with `setenv` before device creation; logged at channel teardown, so the game must be quit normally | per-submission cost on `dxvk-submit`; whether NVK ever throttles |
| `dxvk-cs` runnable time on core 3: samples outside system calls against CPU ticks, per report | already in `[PROF]` and `[THREADS]`; add the difference as one number | when `dxvk-cs` becomes the limit; whether more work can go to core 3 |
