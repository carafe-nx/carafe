# D1. Where Autorun is in the way, and what would need its design changed

## Answer

The orchestrator can be built on top of Autorun. Every control point it needs can be reached today in one of three
ways:

- a `--wrap` of a symbol that one translation unit of `wine-nx-runtime.elf` defines and another one calls;
- a one-file patch in `runtime/patches/autorun`;
- a patch to FEX or DXVK, which Carafe already builds from source (decisions 29 and 30).

No mechanism studied for W3, W4 or W11 needs Autorun's design changed. Four parts of the design do limit the
orchestrator, and each can be changed with a patch:

- **Two Horizon threads per game thread.** The in-process server gives every Wine thread its own server thread.
  Carafe's `0023` pool already replaces those threads under Horizon sync.
- **Windows priorities do nothing.** The server stores them and Horizon never sees them. Every program thread runs at
  priority 59 with 10 ms round-robin.
- **One mutex and one condition variable** guard every server object and every wait.
- **File reads are synchronous** on the calling thread. The SD read cache holds its single lock while a card request
  is in flight.

Two things would be redesigns: splitting the server's single object lock into many, and a thread model other than one
pthread per Wine thread. Neither is needed until a console measurement shows that they cost time: lock waits above
roughly 1 % of the main thread, or a game that runs out of the 96 threads. Leaving Autorun would pay only in two cases:

- Carafe's patches to `horizon.c`, now 16 of 61, can no longer be rebased onto upstream. Upstream changed
  `horizon.c` by 1174 lines between `636913ca` and `a54d5964`.
- A mechanism needs a process layout that Autorun cannot give, such as guest memory in 2 MB blocks.

Neither case holds today. The gain from leaving is zero on its own: it only removes the cost of maintaining patches.

## Evidence

Autorun pinned in Carafe at `636913ca` (paths relative to `external/autorun`, `hw/` = `horizon-wine/source/`), newer
Autorun at `a54d5964` (`D:\Repositories\carafe-nx\autorun`), Carafe at the working tree of
`D:\Repositories\carafe-nx\carafe`.

### How Carafe reaches into the runtime today

- The runtime is one static ELF: Wine's unix side (`dlls/ntdll/unix/*.c`), the Horizon glue (`hw/*.c`), winevulkan's
  unix side, and Mesa with NVK linked from the prebuilt mesa-switch archives (`horizon-wine/CMakeLists.txt:903-968`).
  The following are PE DLLs loaded by Wine and are not part of that ELF:
  - FEX (`libwow64fex.dll`, ARM64 PE);
  - DXVK (`d3d9.dll`, i686 PE);
  - every i386 system DLL.
- Carafe compiles its overlay into that ELF and wraps 13 symbols: `wine_nx_osk_*`, `wine_nx_threads_report_stalled`,
  `wine_nx_thread_report`, `wine_nx_compositor_cursor`, `wine_nx_vk_reserve_window`, `svcArbitrateLock`, `main`,
  `pthread_create` and `util_queue_init` (`runtime/overlay/carafe-overlay.cmake:14-28, 121-123`). It changes Autorun's
  files only through `runtime/patches/autorun/*.patch`. Each patch may touch one file
  (`carafe-overlay.cmake:60-70`), and the cmake script copies and patches that file at configure time.
- Autorun already wraps these symbols itself (`horizon-wine/CMakeLists.txt:837, 853-858, 880-883`):
  - `svcCreateThread`, `svcSetThreadCoreMask`;
  - `virtmemFindAslr`, `virtmemFindCodeMemory`, `virtmemFindStack`, `virtmemAddReservation`,
    `virtmemRemoveReservation`;
  - the newlib malloc family;
  - `__libnx_exception_entry`.

  A second `--wrap` of these symbols is not possible. Code that needs the unwrapped function can still name
  `__real_<symbol>`: the linker resolves it to the original for every object. `hw/thread_profile.c:111-118` does this
  for `svcSetThreadCoreMask`.
- **Limit of `--wrap`.** It changes only undefined references, so a call between two functions of the same file is
  never wrapped. `dlls/ntdll/unix/horizon.c` is 20115 lines, and most of its server and mapping functions call each
  other inside it. Those calls are reachable only by a patch.
- Carafe's existing hooks show both routes:
  - **wraps:** `carafe_lock_boost.c` drops a waiting thread from priority 59 to 58 inside `svcArbitrateLock`;
    `carafe_mesa.c` logs Mesa's queues; `carafe_diag.c:426` notes every native thread start;
  - **a devoptab layer under the runtime's `sdmc:`:** `carafe_overlay.c:1657-1716` installs it in a constructor
    before `main`, so the runtime's SD cache wraps Carafe's device (`hw/runtime.c:3503`).

### Map of the control points

| Point | Where it is | Reachable today | What blocks it, and the kind of change |
|---|---|---|---|
| Wine thread start | `server.c:2021`, `hw/runtime.c:2361` call `wine_nx_thread_register` (`hw/thread_profile.c:294-327`); a server thread registers at `horizon.c:15955` | `--wrap=wine_nx_thread_register`: the handle, TID, kind and TEB of every thread as it starts | nothing |
| Thread names | `thread.c:2183-2188` → `wine_nx_thread_set_name` (`thread_profile.c:222-257`) | `--wrap`; DXVK names its threads (`dxvk-cs`, `dxvk-submit`, `dxvk-queue`, `dxvk-shader-*`) | nothing |
| Core placement | `horizon_pin_current_thread` round-robins new threads over cores 0–2 (`horizon.c:3937-3966`); `wine_nx_thread_balance` moves threads every 2 s (`thread_profile.c:946-1022`, called at `hw/runtime.c:302`); the server thread follows its client (`horizon.c:3972-3980, 11514-11523`) | `--wrap=wine_nx_thread_balance` replaces the balancer; `__real_svcSetThreadCoreMask` places a thread on any core, core 3 included | the wrapper strips core 3 from every other caller (`thread_profile.c:113-118`); only threads named in `thread_profile.h:19-28` go to core 3 (`offload_thread_locked`, `thread_profile.c:137-150`). The orchestrator's own policy needs either its own calls through `__real_` or a patch that replaces the name list with a callback: a patch |
| Priority | program threads 59 (`hw/runtime.c:2350-2356`: "Horizon round-robins only priority 59 on cores 0-2"); server threads 57 (`horizon.c:15960`); audio 56 (`hw/audio_unix.c:369`); graphics workers on core 3 at 63; Carafe's commit thread 44 (`carafe_overlay.c:1705-1713`) | `svcSetThreadPriority` on a registered handle | Windows `SetThreadPriority` reaches the server (`thread.c:2652-2662`), which only stores it (`horizon.c:11911`). A game's own priorities never reach the scheduler. Mapping them is a patch to `horizon.c` (or the orchestrator sets priorities itself) |
| Thread creation | `NtCreateThreadEx` → pthread → `__wrap_svcCreateThread`, which holds `mapping_mutex` around the kernel call (`horizon.c:19700-19747`) | the orchestrator's helper threads: native pthreads (no TEB, may not call Wine, `thread.c:1810-1814` per the cloud study), or Wine threads made by i386 code | **design:** each Wine thread is one pthread plus a server thread; 96 threads per application (Atmosphère PM). Carafe's `0023` pool and `0024`–`0028` (patches) lower the cost; upstream `9b02ed2a` (after the pin) asks the kernel for up to 256 |
| Waits and locks | critical sections, SRW locks and `WaitOnAddress` run in the i386 ntdll and reach `horizon_futex_wait` (`sync.c:219-238`, `horizon.c:13887-13914`); keyed events rendezvous in-process (`sync.c:3783-3815`); events, mutexes, semaphores and `select` run on the calling thread under Horizon sync (`server.c:365-404`, `horizon.c:15892-15945`); everything else goes through the request pipe to a server thread | `--wrap=horizon_futex_wait`, `--wrap=horizon_server_sync_call` (defined in `horizon.c`, called from `sync.c` and `server.c`) let the orchestrator time every wait | every server wait sleeps on one condition variable under one mutex, `horizon_server_objects_mutex` (363 uses in `horizon.c`; `horizon_server_sleep_locked`, `horizon.c:3418-3432`). Splitting it is a **redesign** of the in-process server |
| Yield and sleep | `NtYieldExecution` = `svcSleepThread(-1)` (`sync.c:2434-2448`); `NtDelayExecution` sleeps in the kernel (`sync.c:2476-2545`) | patch, `sync.c` | nothing |
| Guest memory | `NtAllocateVirtualMemory` → `virtual.c` → `horizon_mmap_fixed` (`horizon.c:19751-19810`): heap pages from 2 MB arenas (`horizon_pool.h:39-47`) aliased into the guest range with `svcMapProcessMemory` (`horizon.c:19097-19110`), under `mapping_mutex`; ranges of 256 MB or more are committed on the first fault, 2 MB at a time (`horizon.c:19526-19540, 19593-19640`) | patch, `horizon.c` | whether the kernel maps those aliases with 2 MB blocks is not verified (C5). Guest pages in large blocks would mean changing the alias scheme: a large patch, close to a redesign of the mapping code |
| Native memory | newlib malloc with one lock, wrapped by `hw/native_heap.c` (`CMakeLists.txt:853-858`); FEX allocates through unix calls to it (`hw/fex_unix.c:101-133`) | patch to `native_heap.c`, or FEX patches that pool their own memory | the single lock is a known contention point at start-up (`docs/autorun.md`, Start-up hangs) |
| JIT code memory | `svcCreateCodeMemory` arenas, owner RW and slave RX (`hw/fex_jit.c:52-95`); at most 10 CodeMemory objects for the whole system (`docs/autorun.md`) | FEX patches (Carafe builds FEX) | translation on another core (W10) shares the same arenas, so the 10-object limit does not grow with it |
| File I/O | `NtReadFile` does a regular-file read synchronously on the calling thread, overlapped or not (`file.c:6510-6530`, "async I/O doesn't make sense on regular files"); then the `sdmc:` devoptab: the runtime's SD cache (`hw/sd_cache.c:729-758`) over Carafe's overlay (`carafe_overlay.c:830-846`) over RomFS, save data or the card | Carafe's own overlay layer (no Autorun change); a patch to `sd_cache.c` for its locking | the cache holds `sd_cache_mutex` while it fills a chunk from the card (`sd_cache.c:421-430`): see W11 |
| Wine server | in-process: request handlers are static functions in `horizon.c`, run by a server thread at priority 57 pinned to its client's cores, or on the calling thread for sync and `GET_HANDLE_FD` (Carafe `0021`, `0022`) | patches; the request counters are global and readable (`server.c:357-363`) | the server runs inside the process, so the orchestrator can read its state directly. This helps the orchestrator |
| Presentation | Vulkan present and frame limit in `dlls/win32u/vulkan.c`; compositor thread on core 3 at 63 (`thread_profile.c:320-324`) | Carafe patches `0009`, `0010`, `0042`, `0062` and the wrap of `wine_nx_vk_reserve_window` already reach here | nothing |
| FEX internals | `libwow64fex.dll`, built by Carafe from `395b132f` + `horizon.patch` + `runtime/patches/fex` | FEX patches. The orchestrator talks to FEX through FEX's unix-call table (`hw/fex_unix.c:131-135`, a one-file Autorun patch adds entries) or through shared memory | nothing |
| DXVK internals | i686 `d3d9.dll`, built by Carafe | DXVK patches (`0004`, `0005` already); the runtime passes settings through `dxvk.conf` keys (Autorun `0060`) and environment (`CARAFE_D3D9_STATS`) | DXVK cannot call native code without a unix library (the cloud study's Autorun `0055`, a patch to `virtual.c`). A shared page whose address goes in an environment variable is enough for counters and flags |
| Mesa and NVK | prebuilt static archives from the build image (`CMakeLists.txt:903-968`) | only `--wrap` of their external symbols (Carafe wraps `util_queue_init`, `pthread_create`) | changing NVK itself means building mesa-switch in Carafe's container: a new build step, not an Autorun change |
| System limits | 96 threads (Atmosphère PM); core 3 and priority 63 need the NPDM, which Carafe's loader has (`runtime/loader/carafe-loader.json`); no signals; PMU unknown (C6) | — | outside Autorun |

### What runs in a race, and what that says about the limits

From `D:\debug_switch\carafe\nfsu2-exp9\logs\autorun_runtime.log`, race intervals at 1887 MHz:

- Line 1029: `4w@1 98.7% 44w:dxvk-cs@3 36.9% 40w:dxvk-queue@0 6.9% 104w@0 5.0% 36w:dxvk-submit@2 2.6%
  100w:wine_dsound_mixer@0 1.3%`, 1.53 cores in all. Cores 0 and 2 are each about 90 % idle, and core 3 about 60 %.
- The main thread spends 0.9 % in system calls: 0.7 % in `svc 0x1D`, 0.0 % in the rest (line 1038). The server's
  locks and waits do not show on it. The `[SERVER]` line (line 1030) counts 28375 `select` calls with 40 s of waiting
  in 10 s. These are other threads blocking with timeouts, not CPU time.
- `[BUDGET] threads=47/96 ... pool=2/2/0` (line 995): with Carafe's pool, NFSU2 uses half of the thread limit.

So in NFSU2 the design limits above cost no CPU today. They become relevant once the orchestrator adds threads that
synchronise often (W3 worker, W5 helper), which would make the single server lock and the 59-only round-robin show.

### Differences in Autorun `a54d5964` that matter

48 commits after the pin. The ones that touch the orchestrator's points:

- `9b02ed2a` expands the application's thread limit (`BoostApplicationThreadResourceLimit`). Whether this changes
  anything on stock Atmosphère is not verified (`docs/research/threads/REPORT.md`).
- `565b0ee6` adds a suspend check (an `str` to the interrupt page plus a branch) on every backward conditional branch
  in FEX's JIT (`horizon-wine/fex/horizon.patch`, `DEF_OP(CondJump)`). Today it sits only at block entry (`FEX
  JIT.cpp:769-817`). Every loop iteration of translated code gets two more instructions. That cost is not measured. It
  applies only if Carafe's FEX build takes the newer `horizon.patch`.
- `4c9841d0`, `ff661d9a`, `8302c7b9` read large read-only file views on demand, 512 KB at a time, behind the
  `lazy-file-views` setting (W11).
- `2c25ae70` adds byte-range locks, which Carafe has as `0052`. `2c799250` and `a54d5964` are Carafe's own patches
  `0026` and `0027`, now upstream.
- `horizon.c` gained 1174 lines and lost 97 (`git diff --stat 636913ca a54d5964`), and 16 of Carafe's 61 Autorun
  patches change `horizon.c`. That is the rebase cost the conclusion weighs.

## What this means for the orchestrator

- **Where it lives.** `runtime/orchestrator/`, compiled into `wine-nx-runtime.elf` the way the overlay is.
- **What it uses.** A small set of wraps:
  - `wine_nx_thread_register` and `wine_nx_thread_unregister`, for every thread;
  - `wine_nx_thread_set_name`, for the role of each thread;
  - `wine_nx_thread_balance`, to replace the balancer;
  - `horizon_futex_wait` and `horizon_server_sync_call`, to time waits.
- **What it sets.** Cores and priorities with `__real_svcSetThreadCoreMask` and `svcSetThreadPriority`, for any
  thread, core 3 included, with the same rules as Autorun (priority 63 on core 3 only while the NPDM allows it,
  `nx_thread_helper_available`).
- **One Autorun patch** exports what wraps cannot reach:
  - a callback in `offload_thread_locked` in place of the name list;
  - the server lock's wait time;
  - a mapping of Windows priorities to Horizon priorities (59 ± 1, so as not to break the 59-only round-robin);
  - entries in FEX's unix-call table.
- **FEX and DXVK** take orchestrator hooks as their own patches. DXVK reads a shared page for flags and counters,
  with no unix call.
- **Without core 3** (no forwarder) every rule above still holds with cores 0–2. `wine_nx_four_cores_available()`
  (`thread_profile.c:120-127`) tells which case the game is in.
- **Leaving Autorun** is a decision to take again when one of these is measured:
  - the server lock costs more than 1 % of the main thread;
  - a game runs out of threads with the pool;
  - a rebase onto upstream costs more than a week.

## Needs the console or the lab

| Measurement | How | Decision it switches |
|---|---|---|
| Server lock wait | Patch `horizon.c`: time `pthread_mutex_lock(&horizon_server_objects_mutex)` per thread. Report in `[SERVER]`. Lab for correctness, console race for numbers | Above 1 % of the main thread → design a split lock (the redesign). Below → leave it |
| Wait time per thread | Wrap `horizon_futex_wait` and `horizon_server_sync_call`. Sum blocked time per TID into `[THREADS]` | Which threads wait on which. Input for T2 and for where W3's worker sleeps |
| Effect of 59-only round-robin | Lab: two busy Wine threads on one core at 59, then at 58 and 59. Console: main thread and W3 worker sharing a core | Whether the orchestrator may use priorities other than 59 for program threads |
| Core 3 room | `[THREADS]` with W3's worker placed on core 3 at 63 against cores 0 or 2 at 59 | Where helper threads go with the forwarder and without it |
| FEX doorbell cost (`565b0ee6`) | Build FEX from `a54d5964`'s `horizon.patch`. Count instructions on the hot blocks (`tools/fexcount`) and fps in the race | Whether to rebase FEX now, or carry a patch that drops the check where the game never suspends |
| Thread limit | `[BUDGET]` peak in OpenTTD and NFSU with the orchestrator's helper threads added | Whether `9b02ed2a`'s boost is needed |
