# The Horizon scheduler (C1) and the cost of a hand-off between cores (C3, source side)

## Answer

Horizon schedules by strict priority (0 highest, 63 lowest) with one run queue per core and one global scheduler
lock. A thread runs until it blocks, yields or is preempted by a higher priority. Time slicing exists at one priority
per core only: every 10 ms a kernel thread on core 3 rotates the threads of priority 59 on cores 0–2 and of
priority 63 on core 3. At every other priority, threads of equal priority do not share a core until one blocks. A
thread runs only on cores in its affinity mask; a new thread's mask holds only its ideal core. With a wider mask the
kernel moves runnable threads to idle cores at once, at every scheduler update, not only on yield. Wake-ups go
through the global lock, a per-process tree of waiters and, if another core must reschedule, a GIC software
interrupt. Kernel mutexes hand ownership straight to the highest-priority waiter and lend it their priority
(priority inheritance); condition variables and address arbiters do not lend priority. On the Switch this is not
only a reference: Atmosphère boots mesosphère in place of Nintendo's kernel since 1.2.0, so the code read here is the
kernel Carafe runs on. The timing of a hand-off is not measured: from the source and one console log, a hop on the
same core costs at most about 1 µs, a wake on another idle core an estimated 1.5–5 µs, and a wake that has to wait for
a busy core at its own priority up to 10 ms per thread ahead of it.

## Evidence

Sources: Atmosphère `6b0c077` (`D:\debug_switch\carafe\sources\Atmosphere`; paths below are relative to
`libraries/libmesosphere` unless they start with `mesosphere/`, `stratosphere/` or `docs/`), Eden `58c1e20`
(`carafe-lab/external/eden`), Autorun `636913ca` (`carafe/external/autorun`), console logs
`D:\debug_switch\carafe\nfsu2-exp9\logs`.

### Which kernel runs

- `docs/changelog.md:458,474` (Atmosphère 1.2.0): "Since mesosphere is now always-on". Every release since notes
  "`mesosphère` was updated to reflect the latest official kernel behavior" (`docs/changelog.md:40,74,100…`).
  The source keeps Nintendo's quirks on purpose, for example `include/mesosphere/kern_k_priority_queue.hpp:274`
  ("Nintendo pushes onto the back of the suggested queue") and `source/svc/kern_svc_thread.cpp:93,107,116`.
- Where mesosphère may differ from Nintendo's kernel does not matter for Carafe: a console without Atmosphère cannot
  run Carafe's NSPs. Eden, an independent port, has the same constants (below).

### Priorities and rights

- Priorities 0–63, 0 highest (`libvapours/include/vapours/svc/svc_types_common.hpp:383-384`).
- A process gets a core range and a priority range from its NPDM capability; priorities 0–3 are refused to any
  process (`source/kern_k_capabilities.cpp:65-104`, line 101). `svcCreateThread` and `svcSetThreadPriority` check
  both (`source/svc/kern_svc_thread.cpp:36-40,141-143`).
- Carafe's loader allows cores 0–3 and priorities 28–63 (`carafe/runtime/loader/carafe-loader.json:28-33`; npdmtool
  writes `highest_thread_priority` into the field the kernel reads as the lowest priority, bits 4–9,
  `include/mesosphere/kern_k_capabilities.hpp:69-76`; the reverse order would fail the kernel's `min <= max` check
  at `kern_k_capabilities.cpp:77`, so the reading 28–63 is the only one that boots). Stock nx-hbloader allows cores
  0–2 and priorities 28–59 (`carafe/external/nx-hbloader/hbl.json:30-33`).
- Autorun's priorities: program threads 59 (`horizon-wine/source/runtime.c:2350-2356`), Wine server connection
  threads 57 (`dlls/ntdll/unix/horizon.c:15955-15960`), audio feeder and stall watch 56 (`audio_unix.c:369`,
  `runtime.c:3556`), the 2 ms sampler 36 (`thread_profile.c:580-582`), graphics workers 63 on core 3
  (`thread_profile.c:137-150`). Carafe's lock boost lowers a waiting program thread to 58 for the duration of
  `svcArbitrateLock` (`carafe/runtime/overlay/carafe_lock_boost.c`).

### Run queues

- One global priority queue object with two halves: a scheduled queue per core (threads whose active core is that
  core) and a suggested queue per core (threads that may run there but sit on another core). Pushing a runnable thread
  puts it in the scheduled queue of its active core and in the suggested queue of every other core in its mask
  (`include/mesosphere/kern_k_priority_queue.hpp:247-261`). Each queue level is found with a 64-bit bitmap and
  count-leading-zeros (`:175-184`).
- One scheduler lock for all cores and processes: `KScheduler::s_scheduler_lock`, `s_priority_queue`
  (`source/kern_k_scheduler.cpp:23-25`), a spin lock plus disabled dispatch
  (`include/mesosphere/kern_k_scheduler_lock.hpp:48-92`). Every block, wake, priority or mask change on any core
  takes it.
- On unlock the kernel recomputes the top thread of all four cores (`kern_k_scheduler.cpp:110-145`) and sends a
  software interrupt only to the cores whose top thread changed
  (`include/mesosphere/kern_k_scheduler_impls.hpp:36-41`; the write to `GICD_SGIR` is at
  `include/mesosphere/arch/arm/kern_generic_interrupt_controller.hpp:209-212`). The current core reschedules itself
  on the way out (`kern_k_scheduler.hpp:135-145,186-206`).

### Ideal core, affinity mask, migration

- A thread is created with affinity = its ideal core only (`source/kern_k_thread.cpp:203-207`).
  `svcSetThreadCoreMask` widens it within the process mask (`source/svc/kern_svc_thread.cpp:161-191`,
  `kern_k_thread.cpp:750-849`). If the thread is running on a core the new mask excludes, the call loops until it is
  off that core (`kern_k_thread.cpp:820-845`).
- Idle cores steal work at every scheduler update, not only on yield: for each idle core the kernel takes the
  highest-priority suggested thread that is not the top thread of its own core and moves it; failing that, it takes
  the top thread of a core that has a second runnable thread (`kern_k_scheduler.cpp:147-204`). The only exception is
  a source core whose top thread has priority 0 or 1 (`HighestCoreMigrationAllowedPriority = 2`,
  `include/mesosphere/kern_k_scheduler.hpp:37`; Eden: `src/core/hle/kernel/global_scheduler_context.h:28`).
- A busy core does not pull work from another busy core except in the 10 ms rotation and in the yields below.

### Preemption and the 10 ms rotation

- One DPC thread per core at kernel priority 3. On cores 0–2 it only serves requests; on core 3 it also wakes every
  10 ms and, under the scheduler lock, calls `RotateScheduledQueue(core, p)` for every core with `p` = 59 for cores 0–2
  and 63 for core 3 (`source/kern_k_dpc_manager.cpp:105-106,123-146,150-171`;
  `include/mesosphere/kern_k_dpc_manager.hpp:24-25,32-39`). Eden has the same table, 59/59/59/63 every 10 ms
  (`src/core/hle/kernel/global_scheduler_context.cpp:39-52`, `kernel.cpp:262-268`).
- The rotation moves the first thread of that priority on that core to the back, may pull an equal-priority thread
  from another core's suggested list (unless the next local thread has waited longer), and may pull a higher-priority
  suggested thread (`kern_k_scheduler.cpp:339-421`).
- The timer is one-shot per core, programmed for the next deadline (`source/arch/arm64/kern_k_hardware_timer.cpp:36-53`,
  `include/mesosphere/kern_k_scoped_scheduler_lock_and_sleep.hpp:30-48`). There is no periodic tick on cores 0–2: a
  thread alone on its core is interrupted only by interrupts routed to that core and by the rotation IPI.
- Consequences, from the code above: threads of equal priority other than the rotation priority run in turn only when
  one blocks or yields; a thread at 59 woken on a core where another 59 runs waits for the next rotation (0–10 ms) per
  thread ahead of it, unless an idle core in its mask takes it. Carafe saw exactly this: 48 threads on one critical
  section had 290 waits over 100 ms and one of 1.2 s (`carafe/docs/research/threads/REPORT.md:124-128`).

### The three yields (`svcSleepThread` 0, −1, −2)

`source/svc/kern_svc_thread.cpp:89-118`, values in `svc_types_common.hpp:280-282`.

- All three do nothing, without even taking the lock, if no thread of the process changed state since this thread's
  last empty yield (`kern_k_scheduler.cpp:432-435,470-473,547-550`).
- 0, without core migration: the thread goes to the back of its priority on its core. Lower priorities still do not
  run (`:423-459`).
- −1, with core migration: as 0, then pulls one suggested thread of the same or higher priority from another core onto
  this one, if that thread is not running there (`:461-536`). Wine's `NtYieldExecution` (Sleep(0), SwitchToThread)
  uses this one (`autorun/dlls/ntdll/unix/sync.c:2438-2446`).
- −2, to any thread: the thread gives up its core entirely (active core −1) and, if nothing else is scheduled there,
  pulls the first suggested thread; the yielder is placed again by the next update, possibly on another core
  (`:538-607`).
- A positive value sleeps until `now + ns + 2 ticks` on the one-shot timer (`kern_svc_thread.cpp:94-108`); the tick
  is 19.2 MHz (52 ns).

### Locks, condition variables, inheritance

- `svcArbitrateLock` (`KConditionVariable::WaitForAddress`, `source/kern_k_condition_variable.cpp:112-148`): the
  waiter is added to the owner's held-lock list (`kern_k_thread.cpp:1116-1142`). If its priority is higher, the owner
  and, transitively, the owner's own lock owner inherit it (`RestorePriority`, `kern_k_thread.cpp:1165-1213`).
- `svcArbitrateUnlock` (`SignalToAddress`, `kern_k_condition_variable.cpp:72-110`): the highest-priority waiter
  becomes the owner in the kernel (`RemoveWaiterByKey`, `kern_k_thread.cpp:1236-1291`), its handle is written into the
  user word and it is made runnable. Ownership is handed over: nobody else, including the unlocker, can take the lock
  until the new owner runs and releases it. On a busy core at 59 that is up to 10 ms per thread ahead.
- `svcSignalProcessWideKey` (`Signal`, `:193-217`): waiters are taken in (key, priority) order from a per-process
  red-black tree (`include/mesosphere/kern_k_thread.hpp:173-203`). For each, the kernel tries to take the associated
  mutex word for it (`SignalImpl`, `:150-191`): if the mutex is free the waiter becomes owner and runnable; if held,
  the waiter is queued on the holder as a lock waiter (inheritance applies) and runs only after the holder's
  `svcArbitrateUnlock`. Signalling while holding the mutex therefore costs the waker two system calls and defers the
  wake to the unlock; signalling after unlocking costs one.
- `svcWaitProcessWideKeyAtomic` (`Wait`, `:219-281`) releases the mutex (handing it to its next waiter) and sleeps in
  one call.
- `svcWaitForAddress`/`svcSignalToAddress` (`source/kern_k_address_arbiter.cpp`) keep a priority-ordered tree but no
  inheritance. Autorun's `RtlWaitOnAddress`-style waits use them (`autorun/dlls/ntdll/unix/horizon.c:13892-13913`).

### The wake path, step by step

1. Waker enters the kernel (SVC), takes the global scheduler lock, finds the waiter in the tree, sets it runnable:
   queue insertions on its active core's scheduled queue and every other core's suggested queue
   (`source/kern_k_thread_queue.cpp:25-39`, `kern_k_scheduler.cpp:294-315`).
2. Unlock: recompute top threads for four cores, migrate to idle cores if any (`kern_k_scheduler.cpp:110-207`), DSB,
   one `GICD_SGIR` write if another core's top thread changed (`kern_k_scheduler_impls.hpp:36-41`).
3. Target core: if idle it sits in `wfi` in the idle loop, no deeper power state
   (`mesosphere/kernel/source/arch/arm64/kern_k_scheduler_asm.s:254-272`). The interrupt is read and acknowledged
   at the GIC CPU interface (`source/arch/arm64/kern_k_interrupt_manager.cpp:109-173`), then `ScheduleImpl` saves
   callee-saved and, if used, FPU registers, switches the page table (ASID, no TLB flush) and TLS, restores the new
   thread (`kern_k_scheduler_asm.s:26-244`, `kern_k_scheduler.cpp:209-280`,
   `include/mesosphere/kern_k_process.hpp:381-390`).
4. The woken thread runs on the core it last ran on, unless the update moved it to an idle core.

A woken thread whose priority is not higher than the target core's running thread causes no interrupt: it waits in
the queue.

### Numbers from the console and from sources

- Same core, measured: Wine's in-process server answers `get_handle_fd` in 2 µs and `set_queue_mask` in 4 µs on
  average, client to server thread and back, server work included (`autorun_runtime.log:795,804`). The server
  connection thread follows its client's core mask (`thread_profile.c:73-74`,
  `autorun/dlls/ntdll/unix/horizon.c:3972-3979`) and runs at 57 above the client's 59, so these are two same-core
  wake-and-switch hops of a mutex/condition-variable pipe plus the work: at most about 1 µs (1900 cycles at
  1887 MHz) per hop. Whether every request stayed on one core is not verified.
- Across cores: not measured. Estimate 1.5–5 µs to wake a thread on an idle core: the waker's half of the same-core
  path, one GIC write, a `wfi` exit, the GIC acknowledge on the target and one context switch. Not verified.
- Data between cores: all four A57 cores share one 2 MB L2. A line written by one core and read by another comes from
  the L2 or a snoop of the writer's L1 through it. The measured L2 hit is 34 cycles (`autorun_runtime.log:838`); a
  dirty line in another core's L1 is estimated at 40–100 cycles. Not verified.
- ARMv8.0 cores: no LSE atomics, so contended atomics are LDXR/STXR loops on a line that moves between cores.

### Other facts the orchestrator can use

- User preemption control: if a thread's TLS `disable_count` (offset 0x100,
  `libvapours/include/vapours/svc/arch/arm64/svc_thread_local_region.hpp:24-29`) is non-zero when a reschedule
  interrupt hits it, the kernel pins it to its core, raises it above every priority of its process (28 → 27) and sets
  `interrupt_flag` (`source/arch/arm64/kern_k_interrupt_manager.cpp:187-198`, `kern_k_thread.cpp:566-614`). Until it
  calls `svcSynchronizePreemptionState` (0x36, allowed in Carafe's NPDM, `carafe-loader.json:93`) it may make no other
  system call (`kern_k_thread.cpp:71-100`; `source/svc/kern_svc_synchronization.cpp:119-131`). The pin itself only
  overrides threads of the same process (`kern_k_scheduler.cpp:124-138`); against other processes the raised priority
  27 is what counts.
- Per-thread CPU time without a system call: at each switch the kernel writes `cpu_time − now` into the TLS field
  `thread_cpu_time` (`kern_k_scheduler.cpp:276-279`); adding the current tick gives the thread's CPU time. That the
  user-mode tick (`CNTPCT_EL0`) equals the kernel's is not verified.
- Device interrupts go to the core of the thread that created the interrupt event (`source/kern_k_interrupt_event.cpp:31,37`).
  System modules run on core 3 ([core-3.md](core-3.md)), so their interrupts land there.

## What this means for the orchestrator

- Priority is the only scheduling tool, and the rotation priority is special. On cores 0–2, 59 is time-sliced, 58 and
  above run until they block. Put the main thread alone on its core: either 59 with nothing else at 59 allowed there,
  or 58 pinned so no 59 can take its slices. Autorun keeps it at 59 because a 58 main thread spinning on a lock starves
  a 59 holder on the same core (`runtime.c:2350-2353`); at 58 every wait of the main thread must block, not spin.
- Helper threads that must react within microseconds (a pipeline stage, a server) belong one or two levels above the
  thread that feeds them (57–58), stay short, and block when idle. Bulk work belongs at the rotation priority, or at
  63 on core 3.
- Masks: give workers 0x7 (or 0xF where core 3 is allowed) so idle-core stealing places them for free; give the main
  thread a single core. A worker pinned to one core waits for that core even when others idle.
- Hand-offs cost microseconds, not nanoseconds. With a frame of about 21 ms (47 fps), 100 cross-core wakes a frame
  cost about 0.2–2 % of the main thread if the main thread pays them; a stage design must batch work so that one wake
  carries at least tens of microseconds of work. Prefer a waker that keeps running (signal after unlock, or a
  lock-free queue with a wake only on empty-to-non-empty).
- Kernel mutexes are hand-off locks: a waiter on a busy core at the same priority keeps the lock unowned in practice
  for up to 10 ms. For locks shared by the main thread and workers: wait at a higher priority (the lock boost does
  this), spin briefly before blocking, or make the holder short and the waiter higher.
- Condition variables and address waits lend no priority. A main thread waiting for `dxvk-cs` at 63 on core 3 through
  a condition variable waits behind every system thread there; only a mutex wait lends 59 to the holder.
- Short spin-protected sections in the runtime (FEX's code cache lock, DXVK spin locks) can use the TLS
  `disable_count` to avoid being preempted by sibling threads mid-section, if they make no system call inside.
- The closed loop can read per-thread CPU time from TLS at no cost and per-core idle time with `svcGetInfo`
  ([core-3.md](core-3.md)).

## Needs the console

| Name | How to measure in `[A57]` (user mode) | Decision it switches |
|---|---|---|
| `wake-same-core` | Two threads pinned to core 0, waker 59, sleeper 58 waiting on (a) `svcWaitForAddress`, (b) a libnx condition variable; the waker writes `CNTPCT` into a shared slot then signals; the sleeper reads `CNTPCT` on return. 10 000 rounds, median and 99th percentile, in ticks and cycles. | Whether a same-core pipeline stage (main thread → helper on the same core) is cheaper than a cross-core one. |
| `wake-cross-idle` | As above, sleeper pinned to core 2 with core 2 otherwise idle; waker on core 1. | The minimum work per hand-off for a cross-core stage: if a wake is above 5 µs, batch per draw group; if below 2 µs, per state block is possible. |
| `wake-cross-busy` | Sleeper on core 2 at 59 while a 59 spinner runs there; then sleeper at 58 above the spinner. | Confirms the 0–10 ms rotation wait and that one level above preempts at once; sets the priority rule for stage threads. |
| `line-pingpong` | Two threads pinned to cores 1 and 2 (then 1 and 3) bounce one 64-byte line with plain stores and spin loads, 100 000 round trips timed by `CNTPCT`; also a 16 KB block written on one core and read on the other. | Cost per line handed over; how large a command batch must be before the copy is cheaper than the work moved. |
| `atomic-contended` | Two cores increment one counter with LDXR/STXR (as FEX emits for `lock add`) against one core alone. | Whether shared counters on hot paths (profiler, lock boost counter) must become per-thread. |
| `yield-cost` | 100 000 `svcSleepThread(-1)` on an otherwise idle core, then with one equal-priority thread waiting on another core. | Whether spin-then-yield loops (Wine's Sleep(0)) are cheap enough to keep. |
| `scheduler-lock-contention` | `wake-cross-idle` while two other cores run a tight `svcSleepThread(0)`-with-change loop. | Whether the global scheduler lock makes wake cost grow with other threads' activity. |
