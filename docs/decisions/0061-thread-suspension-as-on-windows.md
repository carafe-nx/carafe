# 61. Suspending a thread works as on Windows

Date: 2026-11-04

Replaces the snapshot of a thread's registers in [31](0031-suspend-and-terminate-threads.md) and the row
`suspend-wakes` of [57](0057-window-hooks-built-in.md).

## Context

Garbage collectors of Unity (IL2CPP, Mono) and other runtimes suspend every thread with `SuspendThread` and read its
registers with `GetThreadContext` to find the objects it still uses. On Windows a thread is suspended wherever it is:
a thread waiting in the kernel stops at once, gives the registers it had when it entered the kernel, and goes on waiting
when it is resumed.

Horizon has no signals, and Autorun's in-process server could not reach a thread blocked in a Horizon wait:

- A 64-bit (ARM64EC) thread waiting in `WaitOnAddress`, an SRW lock, a condition variable or a critical section never
  published its registers, and `GetThreadContext` waited for ever. Cat Mail Co stopped at start-up this way.
- A thread waiting in the server published no registers at all, so `GetThreadContext` failed; the Boehm collector
  retried it a million times and ended the game ("SuspendThread loop failed", reported for MiSide).
- A 32-bit thread gave the registers it had when it started: FEX keeps a WoW64 thread's registers in its own state and
  writes them to the thread's CPU area only when asked, and Wine's wow64 never asked.
- The Experimental row "Stopping waiting threads" (patch `0086`) woke an alert wait with a false alert. It covered one
  kind of wait, made waits end early and could stop a thread inside the x86 emulator.

## Decision

- **A thread in a system call is in the kernel.** The system call dispatcher and the Unix call dispatcher keep the
  caller's registers (the callee-saved ones, the frame, the link register, the return point, the arguments, `fpcr` and
  `fpsr`) in their frame (`0118`).
- **A suspended thread in the kernel is held there.** A thread blocked in a Horizon wait inside a system call marks
  itself blocked (`svcWaitForAddress`, `svcWaitProcessWideKeyAtomic`, `svcWaitSynchronization` and `svcSleepThread` are
  wrapped); the server holds it when it is suspended and publishes the registers of its call at once. The wait goes on.
  A suspended thread that reaches the end of its call, a callback into the program, an APC or a continued context
  holds itself there until it is resumed (`0119`, `0121`).
- **Registers set while it is held are applied** when the thread goes back to the program (`0118`, `0120`, `0121`).
- **The server waits for the thread, not for a timer.** Each hold and each publication signals the thread's hold word,
  and `GetThreadContext` waits on that word.
- **The emulator is left alone.** A call made by FEX for itself (in simulation, in a syscall callback or on the
  emulator's stack) is not a kernel entry of the program; such a thread stops at FEX's next consistent point through
  the ARM64EC doorbell, as before.
- **32-bit threads are suspended through the CPU backend**, as Windows' wow64 does: `NtSuspendThread` of a thread of the
  program calls FEX's `BTCpuSuspendLocalThread`, which stops the thread at a block boundary and writes its registers to
  the CPU area (`runtime/patches/wine-pe/`). `wow64.dll` is built from Autorun's source with these patches
  (`runtime/wine-pe/build-pe.sh`, the `pe` target of `runtime/Makefile`) and replaces the prebuilt one.
- **FEX's thread state lives in the process's virtual memory** (`runtime/patches/fex/0023`). Its interrupt page is
  write-protected to stop a WoW64 thread in translated code; on Horizon FEX's heap is the runtime's own `malloc`, which
  `NtProtectVirtualMemory` does not know, so that suspension failed.
- **A terminated thread is alerted**, so an alert wait ends at once and the thread ends on its way out (`0121`).
- `0086` and the Experimental row `suspend-wakes` are gone; `0087` (a WoW64 context is read only once the thread has
  stopped) is always on. The snapshot of a thread's runtime registers in `get_thread_context` is gone. `0005`, which
  ended every endless alert wait after 250 ms, is gone: it made waits return early and it was what ended threads
  terminated in such a wait.

## Rejected

- **Waking the waiting thread** (`0086`). Waits end early, only alert waits are covered, and the thread may stop inside
  FEX, where its x64 registers are not consistent.
- **Pausing a waiting thread with `svcSetThreadActivity`.** A thread paused in a condition variable or a mutex wait is
  given the mutex by the kernel while it is paused, and every other thread then waits for it.
- **Marking only some waits** (`WaitOnAddress`, keyed events, `Sleep`). A thread waiting inside any other call (video,
  sound, graphics) still hung `GetThreadContext`, and a collector that suspended its producer first deadlocked.
- **`BTCpuGetContext` and `BTCpuSetContext` for other threads.** FEX takes the other thread's JIT lock as if it were its
  own and loads the caller's CPU area into it; after `BTCpuSuspendLocalThread` the CPU area already holds the registers.

## Consequences

- The threads probe of the lab (`suspend` mode, 18 checks) gives the same answers as on Windows, 64-bit and 32-bit:
  ten kinds of wait, a spinning thread, 300 rounds of suspending nine threads with no wait ended early,
  `SetThreadContext` of a spinning and of a sleeping thread, SSE registers that keep their values through 200
  suspensions, and `TerminateThread` of four kinds of wait. In one 32-bit run of the 300 rounds one of the nine
  threads was not woken after its last resume, the lost wakeup of TSO emulation being off. A thread
  terminated in a wait for an event in the server still takes about 250 ms to end; on Windows it ends at once.
- Unity games no longer depend on an Experimental row; Cat Mail Co starts with the default settings and closes from
  the HOME Menu.
- OpenTTD reached its first frame in 19 launches out of 20 without `0005`, in 14–25 s; the one other launch ended
  when Eden crashed before Wine started. The wakeup `0005` covered was lost by FEX's locked instructions, which were no
  full barriers ([62](0062-locked-instructions-are-full-barriers.md)).
- Every system call and Unix call saves 416 bytes of registers and checks one word on the way in and out. In the
  lab's probe a 64-bit system call costs 2–5 % more than with v0.5.2; the 32-bit figures move by more than that from
  run to run. Need for Speed Underground 2 races at 24–47 fps, as with v0.5.2 (24–52).
- Without `0005` a 32-bit program running with FEX's TSO emulation off can still lose a wakeup in its own code:
  Undertale stopped at start-up in two launches out of six with TSO off and in none of four with it on.
