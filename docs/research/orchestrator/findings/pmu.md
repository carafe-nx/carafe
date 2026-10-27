# The A57's performance counters on the Switch (C6)

## Answer

Yes, with one setting on the user's SD card. Atmosphère has an official switch, `enable_user_pmu_access=1` in the
`[exosphere]` section of `sdmc:/exosphere.ini`. At boot the secure monitor passes it to the kernel, and mesosphère then
sets `PMUSERENR_EL0 = 1` on every core, which lets user mode read and program the whole PMU: the cycle counter and six
event counters per core. It is off by default. No system call, debug interface or other setting reaches the PMU; a
kernel patch would only repeat what the switch does, and Carafe cannot ship a kernel anyway. The counters are per core
and the kernel does not switch them with threads, so they count whatever runs on the core. That suits Carafe: the main
thread runs nearly alone on core 1, and the counters can be filtered to user mode. The A57 counts L1 and L2 refills,
L1 TLB refills, mispredicted branches, retired and speculated instructions, but has no front-end or back-end stall
events and no L2 TLB refill event; stalls must be derived. Without the switch, the runtime falls back to timing,
sampling, offline cache simulation of an access trace, and per-block counts from FEX.

## Evidence

Sources: Atmosphère `6b0c077` (`D:\debug_switch\carafe\sources\Atmosphere`), the Cortex-A57 Software Optimization
Guide (`D:\debug_switch\carafe\sources\cortex-a57-optimization-guide.txt`), Linux perf's A57 event tables.

### The path from the SD card to `PMUSERENR_EL0`

- `config_templates/exosphere.ini:14-16,60`: "Key: enable_user_pmu_access, default: 0. Desc: Controls whether
  userland has access to the PMU registers. NOTE: It is unknown what effects this has on official code."
- fusee reads `sdmc:/exosphere.ini` and sets `SecureMonitorConfigurationFlag_EnableUserModePerformanceCounterAccess`
  (`fusee/program/source/fusee_setup_horizon.cpp:517,544-549`; flag defined at
  `libraries/libexosphere/include/exosphere/secmon/secmon_monitor_context.hpp:29,103`).
- The secure monitor reports it to the kernel as the `EnableUserPmuAccess` bit of the kernel configuration
  (`exosphere/program/source/smc/secmon_smc_info.cpp:38,170-172`).
- mesosphère stores it (`libraries/libmesosphere/source/board/nintendo/nx/kern_k_system_control.cpp:436`,
  `kern_secure_monitor.hpp:91`, `include/mesosphere/kern_k_target_system.hpp:33,45,57`) and, on each core at start,
  `if (KTargetSystem::IsUserPmuAccessEnabled()) { SetPmUserEnrEl0(1ul); }`
  (`libraries/libmesosphere/source/arch/arm64/kern_cpu.cpp:517-518`).
- Added in Atmosphère 0.10.0 (`docs/changelog.md:1104-1105`). Whether hekate's `fss0` boot honours `exosphere.ini`
  the same way is not verified.
- `PMUSERENR_EL0.EN = 1` gives EL0 access to the PMU control, event type, counter and enable registers; the interrupt
  enable registers stay at EL1 (ARMv8-A Architecture Reference Manual, PMUSERENR_EL0; not re-read here).

### What else the kernel does with the PMU

- Six event counters (`include/mesosphere/arch/arm64/kern_cpu.hpp:26,94-120`).
- A kernel counter-read interrupt exists only with `MESOSPHERE_ENABLE_PERFORMANCE_COUNTER`, commented out in release
  builds (`include/mesosphere/kern_build_config.hpp:41`; `kern_cpu.cpp:72-110,520-523`). No system call or `svcGetInfo`
  type returns PMU values (`source/svc/kern_svc_info.cpp`, no PMU case).
- The PMU registers are saved and restored only around console sleep
  (`source/board/nintendo/nx/kern_k_sleep_manager.cpp:53-61,107-114`); the context switch does not touch them
  (`mesosphere/kernel/source/arch/arm64/kern_k_scheduler_asm.s` saves only general, FPU and stack registers).
- Neither the secure monitor (no `MDCR_EL3` setup found in `exosphere/`) nor the kernel restricts EL1/EL0 counting
  further; the reset values of `MDCR_EL2`/`MDCR_EL3` that make this so are assumed from the A57's defaults. Not
  verified on the console.

### What the A57 can count

- Linux perf's tables for the A57 (https://github.com/torvalds/linux/tree/master/tools/perf/pmu-events/arch/arm64/arm/cortex-a57-a72):
  - cache: `L1I_CACHE_REFILL`, `L1I_TLB_REFILL`, `L1D_CACHE_REFILL`, `L1D_CACHE`, `L1D_TLB_REFILL`, `L1I_CACHE`,
    `L1D_CACHE_WB`, `L2D_CACHE`, `L2D_CACHE_REFILL`, `L2D_CACHE_WB`, and read/write splits
    (`L1D_CACHE_RD/WR`, `L1D_CACHE_REFILL_RD/WR`, `L1D_TLB_REFILL_RD/WR`, `L2D_CACHE_RD/WR`, `L2D_CACHE_REFILL_RD/WR`);
  - branch: `BR_MIS_PRED`, `BR_PRED`, `BR_IMMED_SPEC`, `BR_RETURN_SPEC`, `BR_INDIRECT_SPEC`;
  - instruction: `INST_RETIRED`, `INST_SPEC`, `LD_SPEC`, `ST_SPEC`, `DP_SPEC`, `ASE_SPEC`, `VFP_SPEC`, `LDREX_SPEC`,
    `STREX_PASS_SPEC`, `STREX_FAIL_SPEC`, `DMB_SPEC`, `DSB_SPEC`, `ISB_SPEC`, `PC_WRITE_SPEC`;
  - memory: `MEM_ACCESS`, `MEM_ACCESS_RD/WR`, `UNALIGNED_LD/ST_SPEC`.
  Plus `CPU_CYCLES` and the cycle counter. No `STALL_FRONTEND`/`STALL_BACKEND`, no `L2D_TLB_REFILL`.
- One implementation-defined stall event: `0x12C DISP_SWDW_STALL`, cycles stalled on FP register forwarding hazards
  (Optimization Guide §4.4, txt lines 1859-1861).
- Event numbers: the common events have the ARMv8 numbers (`L1D_CACHE_REFILL` 0x03, `L1D_TLB_REFILL` 0x05,
  `INST_RETIRED` 0x08, `BR_MIS_PRED` 0x10, `CPU_CYCLES` 0x11, `L2D_CACHE_REFILL` 0x17, the `_RD/_WR` splits from
  0x40). Taken from the ARMv8-A common event list, not re-read here; `PMCEID0_EL0` at run time says which of 0x00–0x1F
  exist, and a sanity loop checks the rest.

### Alternatives already in the runtime

- 2 ms sampler of the busiest threads' PCs (`autorun/horizon-wine/source/thread_profile.c:10-21,580-582`), block dumps
  classified by instruction (`carafe/docs/research/game-speed/REPORT.md:199-207`).
- `[A57]` microbenchmarks with `CNTPCT` at 19.2 MHz (`carafe/runtime/overlay/carafe_diag.c`).
- Per-thread CPU time from `svcGetInfo(ThreadTickCount)` and, without a system call, from the TLS field
  `thread_cpu_time` ([horizon-scheduler.md](horizon-scheduler.md)).

## What this means for the orchestrator

- Treat the PMU as an optional sensor. At start, read `PMUSERENR_EL0` (readable at EL0; behaviour in Eden not
  verified, so probe behind the runtime's exception handling or only on the console). If `EN` is set, own the PMU of
  each core the orchestrator measures: one owner writes `PMCR_EL0` and the event types; nothing else in the system is
  known to use them.
- Attribution: count with the EL1 filter on (`PMEVTYPER.P = 1`) so only user mode counts. Read core 1's counters on
  the main thread itself at frame boundaries (no system call, a few `mrs`), or from a helper pinned to core 1 at a high
  priority that wakes every few milliseconds. The 2 ms sampler cannot read another core's counters. Other threads
  that land on core 1 pollute the count; `[THREADS]` shows how much.
- Six counters at a time: rotate event sets per frame or per second. Set A: cycles, `INST_RETIRED`,
  `L1D_CACHE_REFILL`, `L2D_CACHE_REFILL`, `L1D_TLB_REFILL`, `BR_MIS_PRED`. Set B: `L1I_CACHE_REFILL`,
  `L1I_TLB_REFILL`, `BR_INDIRECT_SPEC`, `LD_SPEC`, `ST_SPEC`, `0x12C`.
- Stalls by arithmetic: memory stall cycles ≈ `L2D_CACHE_REFILL` × (DRAM − L2) + (`L1D_CACHE_REFILL` −
  `L2D_CACHE_REFILL`) × L2, with the `[A57]` latencies (34, about 377); mispredict cycles ≈ `BR_MIS_PRED` × 16–24.
  Overlap makes these upper bounds. That answers T1 (what share of the 67 % waits on DRAM) and feeds W5, W7 and W8.
- Whether Carafe turns the switch on for the user is a product decision (it edits the user's CFW configuration and
  needs a reboot). The orchestrator must work without it.
- Without the PMU: an access trace of the main thread run through a simulator with the A57's geometry (L1D 32 KB
  2-way, L2 2 MB 16-way, 64-byte lines, TLB 32/1024) gives miss counts offline; FEX can count executions per block
  and indirect targets per site (exact instruction mix and mispredict estimates); A/B runs behind Experimental rows
  give end-to-end effect.

## Needs the console

| Name | How to measure in `[A57]` (user mode) | Decision it switches |
|---|---|---|
| `pmu-probe` | Read `PMUSERENR_EL0`; if `EN`, read `PMCR_EL0.N` and `PMCEID0/1_EL0`; log them. Console run with `enable_user_pmu_access=1` set for that boot. | Whether the PMU path is used at all, and which common events exist. |
| `pmu-sanity` | On a pinned thread: cycles over the `add` chain of `[A57]` must equal its tick-based cycles; `INST_RETIRED` must equal the loop's instruction count; `L1D_CACHE_REFILL` per step on the 4 MB chase must be about 1; `L1D_TLB_REFILL` on `tlb-ladder` ([memory-pages.md](memory-pages.md)) must step at N = 32. | Trust in the counters and in the event numbers. |
| `pmu-main-frame` | Sets A and B above, read on the main thread at each present, 600 frames each in the race; per frame: IPC, refills per 1000 instructions, mispredicts per 1000. | T1's split of memory time (DRAM against L1/L2), hence the priority of the run-ahead helper (W5) against instruction choice (W7); large pages (W8) if `L1D_TLB_REFILL` exceeds about 5 per 1000 instructions. |
| `pmu-dxvk-cs` | Set A on core 3 with `dxvk-cs` there, EL1 filtered. | Whether the graphics stage is memory-bound, which decides whether it gains from another core at all. |
