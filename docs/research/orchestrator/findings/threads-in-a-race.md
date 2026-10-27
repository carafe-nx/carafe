# T2. Every thread in a race: what it does, what it waits on, who wakes it

## Answer

In the race the main thread (`4w`, core 1) is busy 94–99 % and waits for almost nothing: 1.7 % of its samples are in
system calls, most of them short signals to other threads, 0.4 % a wait inside DXVK and 0.1 % GPU memory mapping.
Every other thread waits for the main thread. `dxvk-cs` (core 3) is 31–40 % busy and spends 31–57 % of its samples
asleep in `NtWaitForAlertByThreadId` until the main thread hands it the next command chunk; `dxvk-queue` (core 0,
6–8 %) sleeps half its samples on the GPU's fence and 40 % on the submission queue; `dxvk-submit` (core 2, 2–3 %),
the game's audio thread (core 0, 5–6 %, polling with `Sleep`), Wine's DirectSound mixer (1.3–1.6 %) and one more game
thread (0.5 %) make up the rest. The registered threads use 1.5 of the four cores, the GPU is 46–73 % busy, and core 0
(about 85 % idle), core 2 (about 95 %) and core 3 (about 60 %, minus what the system uses there) are free. The frame
flows one way: main thread (game, D3DX, DXVK's Direct3D 9 front end) → `dxvk-cs` (DXVK context, Vulkan through
winevulkan to NVK) → `dxvk-submit` (queue submit and present) → GPU → `dxvk-queue` (fence wait, clean-up). Nothing
downstream holds the main thread back, so the frame is the main thread's time. Not measured: about 27 native Horizon
threads that the profiler does not register (the in-process Wine server, Mesa queues, the presenter, input, audio
output); the Wine server's own thread used about 0.9 % of a core.

## Evidence

Run `D:\debug_switch\carafe\nfsu2-exp9` (2026-10-27, 1887 MHz, handheld), `logs\autorun_runtime.log`; the same lines
are in `logs\SPEED2_profiler.log`, 43 lines earlier. Steady race windows: the 16 reports whose main thread is over
90 % x86 (lines 1027–1846; `[THREADS]` on lines 1029, 1081, 1131, …, 1844; `[PROF]` blocks after each). The windows
right after a load (lines 997, 1491, 1951) show the same threads with the main thread at 93–97 %. Replications: exp7
(`nfsu2-exp7\logs\autorun_runtime.log`, 15 steady windows, lines 873–1330 onward) and exp5 (46 windows); every
thread's load agrees within 1–3 points. Scripts: `D:\debug_switch\carafe\tools\orchestrator\windows.js` (parses each
report window; `agg <thread> <table>` sums a table over the steady windows, weighted by samples), `sym.sh`
(`aarch64-none-elf-addr2line` on `runtime/build/wine-switch/wine-nx-runtime.elf`, which matches this run: the log's
`threads started at runtime+0x3a7c70` resolves to `log_flusher`, and every system call site resolves to its libnx or
Horizon wrapper), `..\pesyms.js` on the i386 `d3d9.dll` (`d3d9.dll+0x1bf4f0` = `dxvk::thread::threadProc`, the entry
the DXVK threads were started at: `[WOW64 THREAD] entry=0xfc33f4f0`, base `0xFC180000`).

What the profiler can and cannot see (`external/autorun/horizon-wine/source/thread_profile.c` with patches
`0050`, `0053`, `0055`, `0058`):

- `[THREADS]` lists registered threads only: Wine threads (`w`), their server connection threads (`s`) and the
  compositor (`c`), at least 0.5 % and at most 12. Busy is the kernel's tick count over the interval; `@N` is the core
  from `svcGetThreadCoreMask` (`-1`: no single core).
- `[PROF]` samples the four busiest registered threads that used at least 3 % of a core, every 2 ms, running or
  blocked. A thread under 3 % (`dxvk-submit`, the DirectSound mixer) has no `[PROF]` lines.
- `svc` is the system call number read at the PC. Numbers checked against libnx's `svc.h` in `carafe-runtime:latest`:
  `0x06` QueryMemory, `0x0B` SleepThread, `0x0C`/`0x0D` Get/SetThreadPriority, `0x18` WaitSynchronization, `0x1A`
  ArbitrateLock, `0x1B` ArbitrateUnlock, `0x1C` WaitProcessWideKeyAtomic (condition variable wait), `0x1D`
  SignalProcessWideKey, `0x21` SendSyncRequest (IPC to a service, here `nvhost`), `0x28` ReturnFromException, `0x34`
  **WaitForAddress** (not WaitSynchronization), `0x35` SignalToAddress, `0x73`/`0x75`/`0x77`/`0x78` code memory
  mapping (FEX's code cache).
- "who wakes it" below comes from the wait's caller chain and the waker's own signal calls; a wake-up by another
  thread is a short system call that the 2 ms sampler rarely catches, so the wakers are inferred from code, not
  counted.

### The threads

Busy is the range over the 16 steady windows (mean). The split is the `[PROF]` average over the windows it was
sampled in. Svc entries are shares of that thread's samples, caller from `sym.sh`.

| thread | role | core | busy % | x86 / pe / native / svc | waits in | woken by |
|---|---|---|---|---|---|---|
| `4w` | game main thread: game code, the D3DX in the `.exe`, DXVK's Direct3D 9 front end | 1 | 93.7–98.7 (97.4) | 96.3 / 0.7 / 1.1 / 1.7 | `0x1D` 0.66 % from `__syscall_cond_signal` ← `horizon_pipe_write_r` ← `_write_r`: it signals a waiter; `0x34` 0.43 % in `horizon_futex_wait` ← `NtWaitForAlertByThreadId` (a wait inside DXVK, up to 2.6–3.4 % in windows 1690 and 1846); `0x1C` 0.23 % in `horizon_server_sleep_locked` ← `horizon_server_handle_select` (a Wine server wait); `0x21` 0.11 % in `nvioctlNvhostAsGpu_MapBufferEx`/`UnmapBuffer` ← `nouveau_horizon_va_*` (GPU memory mapping, 1.2–2.5 % in the windows after a load) | nobody: it never sleeps long |
| `44w` `dxvk-cs` | DXVK's command stream: `DxvkContext` (descriptor sets, vertex buffer bindings, `commitGraphicsState`), Vulkan calls through winevulkan's WoW64 thunks into NVK | 3, priority 63 (`[CORES] 44 dxvk-cs: core 3, priority 63`, line 716) | 31.4–40.2 (36.7) | 30.8 / 2.4 / 25.1 / 41.5 | `0x34` 40.6 % in `horizon_futex_wait` ← `NtWaitForAlertByThreadId` ← `wine_nx_do_syscall`, from `dxvk::thread::threadProc`: an empty chunk queue; `0x1A` 0.4 % (lock contention) | the main thread when it dispatches a chunk (DXVK's condition variable → `RtlWakeAddressSingle` → `NtAlertThreadByThreadId` → `SignalToAddress`; inferred from DXVK and Wine) |
| `40w` `dxvk-queue` | DXVK's submission tracker: waits for each command list's fence, then frees and resets (`nvk_ResetDescriptorPool`, `_free_r`) | 0 | 6.0–8.3 (7.3) | 1.6 / 0 / 4.8 / 93.4 | `0x18` 50.1 % in `eventWait` ← `_nvFenceEventWaitCommon` (the GPU's fence); `0x34` 40.0 % in `NtWaitForAlertByThreadId` (nothing submitted); `0x21` 2.6 % `nvioctlNvhostCtrl_EventWaitAsync` | the GPU fence's kernel event; `dxvk-submit` when it queues a command list |
| `36w` `dxvk-submit` | `vkQueueSubmit` and `vkQueuePresentKHR` | 2 | 2.1–3.1 (2.6) | not sampled in exp9; sampled once in exp7 (line 1356, a window where it was 11 % busy): 0.3 / 0 / 2.8 / 96.8 | exp7: `0x34` 93 % in the same alert wait as `dxvk-cs`; `0x21` about 3 % (`nvhost` ioctls of the submit) | `dxvk-cs` when it flushes a command list (DXVK's design; the alert wait is consistent with it) |
| `48w`, `52w`, `56w` `dxvk-shader-l/h` | DXVK's pipeline compilers | any of 0–2 (affinity 0x7, lines 756–758) | 12–21 each while loading (line 794), ≤ 1.8 in the first race window, then under 0.5 | 0 / 0 / 0 / 99.8–100 | `0x34` in `NtWaitForAlertByThreadId` | `dxvk-cs` when a pipeline is missing |
| `104w` → `116w` → `128w` | the game's audio thread, created again for each race (entry `exe+0x3263c7`, lines 772, 964, 1490, 1900; its frames are `exe+0x326275<exe+0x3263c1`); runs the game's sound code in `0x720000–0x740000` (an SSE resampling loop at `0x7337d0`, a hand-written decoder at `0x734aff`; the meaning is inferred) | 0 | 4.8–6.5 (5.2–6.1) | 4.9–6.0 / 0 / 0.3 / 93.4–94.6 | `0x34` 93–95 % in `horizon_futex_wait` ← `NtDelayExecution`: `Sleep` | its own timeout (polling) |
| `100w` → `112w` → `124w` `wine_dsound_mixer` | Wine's DirectSound mixer, recreated with the game's sound device each race | 0, then 1, then 2 | 1.3–1.6 (1.4) | not sampled | (not sampled) | a timer or the audio device (Wine's dsound; not verified) |
| `28w` | a game thread from start-up (entry probably `0x75f617`, the C runtime's thread start; not verified) | 0 | 0.5 | not sampled | (not sampled) | not known |
| `0s` | a server connection thread | 1 | 0.5–0.9, in a few windows | not sampled | | |
| `84w`, `80w` (menus only) | the menu instances of the audio thread and the DirectSound mixer | 0, 2 | 1.7–2.0, 1.1–1.2 | | | |

Threads the profiler does not register, so neither `[THREADS]` nor `[PROF]` shows them: the in-process Wine server
(`horizon_server_thread` and two `horizon_server_pool_thread`s, lines 62, 358–359), the server connection threads'
native side, `presenter_thread` (line 73), `wine_nx_input_thread` (line 731), `usd_clock_thread` (line 69),
`log_flusher` (line 1), Mesa's `disk$` and `gl` queues (lines 74–76, 218, 405), the profiler's sampler and the audio
output. `[BUDGET] threads=47/96` (line 1433) against 20 registered threads leaves about 27 such threads. The Wine
server thread's own work is in `[SERVED]`: `busy_ms` grew from 1461 to 1859 between lines 1434 and 1895 (about 46 s),
0.9 % of a core.

### What the main thread waits on

Its system-call samples in the steady race, `agg 4w svc`: `0x1D` 0.66 %, `0x34` 0.43 %, `0x1C` 0.23 %, `0x21` 0.11 %,
`0x35` 0.02 %; total 1.66 % (exp7 1.3 %, exp5 1.4 %). Its pe and native time (0.7 % and 1.1 %) is the WoW64 bridge
(`libwow64fex.dll+0x106c00`), `wine_nx_do_syscall` and `__wine_syscall_dispatcher`. So the main thread neither waits
for `dxvk-cs`, nor for the GPU, nor for the audio threads in any measurable way. Its `0x34` wait is the only
throttle: a wait through `NtWaitForAlertByThreadId`, which in DXVK is either the CS queue being full or the frame
latency wait at `Present` (which one is not visible in the log). It rises to 2.6–3.4 % only in two windows where the
main thread itself was at 93–94 %.

### What the others wait on, and the frame's chain

- `dxvk-cs` keeps up: idle 31–57 % of its samples, so it executes each chunk faster than the main thread produces
  the next. Its busy time is DXVK's context work in translated code (31 %, top `DxvkCsThread::threadFunc` and
  `updateDescriptorSetsBindings`), native NVK and Wine code (25 %: `nvk_CmdDrawIndexed`, `nvk_CmdBindDescriptorSets2KHR`,
  `nvk_AllocateDescriptorSets`, `vkUpdateDescriptorSets` thunks, newlib `malloc`/`free` and `mutexUnlock` at 2.1 %), the
  WoW64 bridge for every Vulkan call (`0x3c0000` 4.0 % plus pe 2.4 %), and exclusive load/store pairs of reference
  counts (9.8 % of its hot samples; T1).
- `dxvk-queue` waits for the GPU about half its samples: the GPU is behind the CPU only in the sense that each
  command list takes time to finish; with the GPU 46–73 % busy (`[CLOCKS]` after each window), the GPU idles 27–54 %.
- The chain per frame: main thread → (`EmitCs`, chunk dispatch) → `dxvk-cs` → (command list flush) → `dxvk-submit`
  → GPU → `dxvk-queue` → frame done, back to the main thread's latency wait at `Present`. Every arrow except the last
  is one-way and asynchronous; the last costs the main thread 0.4 %.

### The Wine server traffic

`[SERVER]` per steady window (requests from all threads; `windows.js`, the top 8 by time in each window): about 68000
requests per 10 s, 448 frames, so about 150 a frame:

| request | a frame | each | note |
|---|---|---|---|
| `select` | 62 | 1.8 ms | waits on objects, mostly other threads blocking (the average is a wait time, not work) |
| `release_mutex` | 55 | 0.4 µs | a Win32 mutex taken and released about 55 times a frame; which threads use it is not recorded |
| `set_queue_mask` | 18 | 4.5 µs | message polling (`PeekMessage`) |
| `event_op` | 6.7 | 9.6 µs | |
| `set_cursor`, `get_message`, `get_window_parents`, `get_window_rectangles` | 2–3 each | 12–27 µs | cursor and window queries |

Work in requests other than `select` is about 0.3 ms a frame, 1.4 % of a 22 ms frame, on whichever threads make
them. The main thread's pe, native and system call time together (3.5 %) bounds its part.

### Where the free capacity is

| core | registered load in the race | free |
|---|---|---|
| 0 | `dxvk-queue` 7.3 + audio 5.2–6.1 + `28w` 0.5 + DirectSound mixer in some races 1.4 | about 85 %, less unregistered threads |
| 1 | main thread 97.4 + `0s` 0.5 | none |
| 2 | `dxvk-submit` 2.6 + DirectSound mixer in some races 1.6 | about 95 %, less unregistered threads |
| 3 | `dxvk-cs` 36.7 | about 60 %, less the system's share of core 3 (C2) |

All registered threads: 1.45–1.57 cores (`[THREADS] … use 1.52 cores` on average). The program sees three processors
(`[INIT] processors=3`, line 68). Shader compilers are idle in the race.

## What this means for the orchestrator

- Speeding up any thread other than the main thread does not raise the frame rate in this race; only taking work
  off the main thread does. The receivers are ready: about 0.6 of core 3 next to `dxvk-cs`, and most of cores 0 and 2.
- The cheapest place for moved Direct3D work is core 3 next to `dxvk-cs`: it is idle 31–57 % of its time and already
  owns the Vulkan side. Moving the Direct3D 9 front end (17.5 % of the main thread) there costs the main thread only
  the recording; whether core 3 can take 17.5 % more depends on what the system leaves (C2) and on the cost of the
  extra hand-offs (C3).
- Shared reference counts between the main thread and `dxvk-cs` already cost 100–135 cycles per atomic (T1); a
  design that moves more work across cores should give objects one owning core or count references per thread.
- The audio thread polls with `Sleep` and the game runs its sound decoding and mixing there; it is small (5–6 %) and
  can stay on core 0.
- The frame pacing is DXVK's: the main thread's only wait (0.4 %) is inside DXVK. A frame pipeline across cores keeps
  that point as the place where the orchestrator measures how far ahead the CPU runs.
- Before the orchestrator gives cores away it needs the unregistered threads' load, the system's share of core 3, and
  per-thread server traffic (below).

## Needs the console or the lab

1. **Load of every thread (profiler, lab then console).** Register native threads too (wrap libnx `threadCreate` and
   the pthread entry in the runtime, as `wine_nx_thread_register( 'n', … )`), and add each core's idle ticks from
   `svcGetInfo( InfoType_IdleTickCount, …, core )` to `[THREADS]`. Gives the real free share of cores 0, 2 and 3,
   including the system's use of core 3. It switches how much work the orchestrator may place on each core, and
   whether core 3 can take the Direct3D 9 front end (W3) or only part of it.
2. **Who makes the server requests (profiler, lab).** Count `[SERVER]` requests per thread and type. It switches
   whether the main thread's message polling and the 55 mutex operations a frame are worth moving or batching (they are
   at most 1–2 % of the main thread).
3. **Hand-off and wake-up cost between cores (`[A57]`, console).** A ping-pong through `WaitForAddress` and
   `SignalToAddress` between cores 1 and 3, and between cores 1 and 0, plus one cache line written by one core and
   read by the other. It switches the batch size of any queue that moves work off the main thread (W3, W4): a wake-up
   that costs tens of microseconds needs chunks of hundreds of calls.
4. **Wake-ups a frame (profiler, console).** Count `NtAlertThreadByThreadId` and `SignalToAddress` per thread per
   window. Gives how often the main thread wakes `dxvk-cs` today. It switches whether DXVK's chunk size needs tuning
   before more work is put behind the same queue.
5. **Sampler threshold (profiler).** Sample threads from 1 % instead of 3 % for one run, so `dxvk-submit`, the
   DirectSound mixer and `28w` get `[PROF]` lines. It switches nothing large; it closes the rows marked "not sampled".
