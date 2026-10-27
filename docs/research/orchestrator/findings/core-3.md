# Core 3 (C2)

## Answer

Core 3 is the system's core. Every Atmosphère system module starts its main thread there and most may use no other
core; Nintendo's modules are reported to do the same (not verified in source). Their threads run at kernel priorities
15–58, so an application thread at 63 gets exactly what they leave, shared every 10 ms with other priority-63 threads
on core 3. The kernel treats docked and handheld alike; any difference is in how much the system modules do. Carafe
already has the rights the console allows: its NSP's NPDM grants cores 0–3 and priorities 28–63, so core 3 is
available in every Carafe game, with or without Autorun's forwarder. "Owning" more of core 3 means preempting system
threads, which risks input, sound, file and GPU-service latency, or moving system modules away, which only shifts
their load onto the game's cores. The free share is not measured. Known bounds: in the October race run `dxvk-cs` used
35–40 % of core 3 with no visible harm, so the system took at most 60–65 % then; a homebrew port reports core 3 about
33 % busy before it added its own thread. The measurement below gives the share directly from the kernel's idle
counter.

## Evidence

Sources as in [horizon-scheduler.md](horizon-scheduler.md): Atmosphère `6b0c077`, Autorun `636913ca`, Carafe tree,
console logs `D:\debug_switch\carafe\nfsu2-exp9\logs`.

### What runs on core 3

- Every Atmosphère system module's NPDM has `default_cpu_id: 3`. Of the 15 whose core range is in the list form the
  script read, 12 allow core 3 only; `dmnt`, `dmnt.gen2` and `fatal` allow 0–3 (`stratosphere/*/*.json`, all 23
  files; the core range of `sm`, `spl`, `boot`, `fs`, `loader`, `pm`, `ncm`, `ams_mitm` is written in another form
  and was not read). Examples, main
  thread priority: `sm` 27, `spl` 27, `boot` 27, `ams_mitm` 43, `fs` 45, `loader`, `pm`, `ncm`, `ro`, `erpt`, `pgl`,
  `jpegdec` 49, `creport` 44, `htc` 38, `LogManager` 38, `fatal` 15.
- Thread priorities inside system modules (`libraries/libstratosphere/include/stratosphere/ams/impl/ams_system_thread_definitions.hpp:29-178`)
  are in `os` units; the kernel priority is the value + 28
  (`libraries/libstratosphere/source/os/impl/os_thread_manager_impl.os.horizon.cpp:27`,
  `os_thread_manager_impl.os.horizon.hpp:24`). In kernel priorities: `gpio` interrupt handler 16, `hid` IPC server
  18, `bpc` 32, `powctl` 37, `sdmmc` device detector 39, `fs` workers 44–47 and background 58, `ldr`/`pm`/`ncm`/
  `settings`/`ns`/`erpt` 49, `socket` resolver 57. These tables are Atmosphère's copies of the priorities of the
  modules it reimplements or talks to; that Nintendo's own `hid`, `audio`, `nvservices`, `vi`, `am` use the same
  values and core is not verified.
- Which Atmosphère modules replace Nintendo's at boot by default is not verified per module (`fs.json` exists, but
  Atmosphère is not known to replace FS by default).
- Device interrupts are bound to the core of the thread that creates the interrupt event
  (`libraries/libmesosphere/source/kern_k_interrupt_event.cpp:31,37`). System modules on core 3 therefore take their
  device interrupts on core 3.
- The kernel's preemption thread lives on core 3 at kernel priority 3 and wakes every 10 ms to rotate all cores'
  queues (`libraries/libmesosphere/source/kern_k_dpc_manager.cpp:123-146,160-161`).
- `svcCallSecureMonitor` from any core other than 3 panics the secure monitor (Switchbrew, SVC page,
  https://switchbrew.org/wiki/SVC): the system's secure calls are tied to core 3 as well.
- Audio mixing for `AudioRenderer` runs on the audio DSP, not on the A57 cores, according to Eden's model of the
  audio module (`carafe-lab/external/eden/src/audio_core/adsp/adsp.h:24-41`, "a 32-bit Linux4Tegra kernel …
  launched with the sysmodule"). The system's CPU side of audio still runs on core 3. Not verified on hardware.
- Requests the game itself causes are served on core 3: every NVK/nvservices ioctl (submits, fences), `vi` for
  presentation, `hid` for input, `fs` for the SD card. The system's load on core 3 therefore grows with the game's
  frame rate and loading.

### What priority 63 means there

- Core 3's rotation priority is 63, cores 0–2's is 59 (`libraries/libmesosphere/include/mesosphere/kern_k_dpc_manager.hpp:24-25,32-39`;
  Eden `src/core/hle/kernel/global_scheduler_context.cpp:39-52`). A thread at 63 on core 3 runs only when no thread
  of priority 0–62 there is runnable, and shares the rest in 10 ms turns with other threads at 63 on that core.
- At 59–62 on core 3 a thread is still below every system thread listed above (the lowest is 58), so it gets the same
  share as at 63; it only stops sharing with other 63 threads, because 59–62 are not rotated on core 3.
- With a mask of 0x8 a thread never leaves core 3, even when cores 0–2 idle (`kern_k_scheduler.cpp:147-204` moves
  threads only within their mask).

### Rights in the NPDM

- Carafe's loader NPDM: cores 0–3, priorities 28–63, main thread 44 on core 0
  (`carafe/runtime/loader/carafe-loader.json:7-8,28-33`), turned into `exefs/main.npdm` for every NSP
  (`carafe/runtime/Makefile:82-88`). Stock nx-hbloader and hacBrewPack's template: cores 0–2, priorities 28–59
  (`carafe/external/nx-hbloader/hbl.json:30-33`, `carafe/external/hacbrewpack/template/npdm.json:27-30`). Autorun's
  forwarder hbloader: cores 0–3, 28–63 (`autorun/horizon-wine/hbl/hbl.json:30-33`). An NRO inherits the rights of the
  loader process that runs it.
- The kernel takes the core range and priority range from the capability and refuses priorities 0–3
  (`libraries/libmesosphere/source/kern_k_capabilities.cpp:65-104`). Atmosphère's loader checks the ACI capability
  against the ACID (`stratosphere/loader/source/ldr_capabilities.cpp:80-98`) and, for applications, verifies the ACID
  signature of the base NPDM unless program verification is off (`stratosphere/loader/source/ldr_meta.cpp:147-172,290-300`).
  Installed homebrew NSPs rely on the usual signature patches for this; how Carafe's users have it set up is not
  verified here.
- The pin of user preemption control raises a thread to 27, one above the process's best priority
  (`libraries/libmesosphere/source/kern_k_thread.cpp:598-600`): on core 3 such a thread would beat every system
  thread except those at 15–27 (`fatal`, `gpio`, `hid`, `sm`, `spl`, `boot`).

### What Autorun's forwarder changes

- `wine_nx_four_cores_available` asks the kernel for the process core and priority masks and requires core 3 and
  priority 63 (`autorun/horizon-wine/source/thread_profile.c:120-127`, `thread_profile.h:30-33`). Without them it logs
  "core 3 unavailable; reinstall the Autorun forwarder" (`thread_profile.c:129-135`). The console run logged
  `[CORES] graphics workers may use core 3 at priority 63` (`autorun_runtime.log:48`).
- Every other `svcSetThreadCoreMask` in the runtime has core 3 removed by a wrapper (`thread_profile.c:113-118`).
- `dxvk-cs`, `wined3d_cs`, `vkd3d_queue` and the compositor get mask 0x8 and priority 63 when named or registered
  (`thread_profile.c:137-150,247-251,320-323`, `thread_profile.h:19-22`; log line 716:
  `[CORES] 44 dxvk-cs: core 3, priority 63`). DXVK's shader compilers get mask 0x7, priority unchanged
  (`thread_profile.c:167-182,238-246`; log line 756). Carafe documents the same (`carafe/docs/autorun.md:382-385`).

### Measured use

- Race, handheld, 1887 MHz: `dxvk-cs@3` 36.6 %, 39.8 %, 33.1 % of core 3 in three 10 s reports, main thread 94–99 % of
  core 1, all threads 1.46–1.55 cores (`autorun_runtime.log:1231,1545,1844`; `[CLOCKS] … handheld` lines). In menus
  `dxvk-cs` used 5–9 %.
- A third-party report for a homebrew GTA port: core 3 about 33 % busy without its patch, about 66 % with one game
  worker moved there, slightly higher fps; letting every game thread use core 3 caused timing-sensitive crashes
  (https://github.com/PalindromicBreadLoaf/ARMSX2-NX/issues/12). How that load was measured is not stated.

### Handheld and docked

- Nothing in the kernel depends on the mode. The CPU clock is the user's (Carafe never sets clocks). What the system
  modules do on core 3 in each mode (HDMI output, `vi`, docked GPU clocks giving more frames and more submits) is not
  measured. The October runs were all handheld.

## What this means for the orchestrator

- Core 3 is a scavenger core: plan it as "the time the system leaves", 35–90 % of a core (range from the two
  observations above, to be replaced by the measurement), with jitter up to the length of the longest system burst.
  Work there must tolerate being delayed by milliseconds.
- Good tenants at 63 on core 3: translation ahead of the game, shader and pipeline compilation, a memory run-ahead or
  prefetch helper (the L2 is shared by all four cores, so data a helper on core 3 brings in serves the main thread on
  core 1), the profiler's analysis, decompression of SD reads. Each at 63 gets 10 ms turns with the others.
- Bad tenants: anything the main thread waits for within the frame, unless the wait has slack. `dxvk-cs` is one:
  when the main thread waits on it (a full command queue, a query, a lock), it waits behind the system. The
  orchestrator should measure those waits (T2) and move the stage back to cores 0–2 at 58 when they grow.
- Do not raise our threads above 58 on core 3 for long: the system's input, file, GPU and display services would
  wait, and the game's own GPU submissions are among them. A short, bounded burst at 59–62 gives nothing over 63 except
  leaving the 63 rotation.
- Owning core 3 for good is not available beyond what Carafe has. Moving system modules off core 3 (Atmosphère can
  load an override `exefs/main.npdm` from the SD card, `stratosphere/loader/source/ldr_meta.cpp:223-232`) would put
  their load on the game's cores and may break modules that create threads on core 3 explicitly; changing the
  rotation needs a custom kernel the NSP cannot ship. Rejected.
- Without core 3 (an Autorun host with stock hbloader rights, or a future host), the orchestrator reads the process
  masks at start and runs a three-core plan: the main thread pinned alone on one core at 58 or 59, the graphics
  stage and compilers on the other two at 59 with masks 0x6, and no work that needs a fourth core.

## Needs the console

| Name | How to measure in `[A57]` (user mode) | Decision it switches |
|---|---|---|
| `core3-idle` | A probe thread with mask 0x8 sleeps 1 s, then reads `svcGetInfo(InfoType_IdleTickCount, INVALID_HANDLE, 3)` and each of our threads' `svcGetInfo(InfoType_ThreadTickCount, handle, 3)`. System share = 1 − idle − ours. Log every report in menus, loading and the race, with `four-cores` on and off, handheld and docked. | How much work the orchestrator may plan on core 3, per mode. If the system share in the race exceeds 50 %, keep only bulk work there. |
| `core3-jitter` | A thread with mask 0x8 at 63 sleeps 1 ms in a loop and records the oversleep (`CNTPCT` after wake − deadline), histogram over 30 s, race and menus. Repeat at 59. | Whether a latency-bound stage (e.g. the Direct3D front end) may live on core 3, and at which priority. |
| `core3-bursts` | A thread with mask 0x8 at 59 spins reading `CNTPCT`; every gap above 20 µs is a system burst. Count and length distribution per second. | The longest delay a core-3 tenant must tolerate; sizes the slack of any queue between the main thread and a core-3 stage. |
| `core3-main-waits` | Count and total time the main thread spends waiting on `dxvk-cs` (DXVK's queue wait and synchronisation points), with `dxvk-cs` on core 3 at 63 and on cores 0–2 at 58. | Where the graphics stage goes. |
