# 31. SuspendThread and TerminateThread work on Windows threads

Date: 2026-10-20

## Context

Autorun's in-process server stops a running thread only at a safe point: an ARM64EC thread at the doorbell Box64
checks, any thread when it waits in the server. A 32-bit program runs under FEX through WoW64, has no doorbell and
mostly does not wait in the server: `Sleep` sleeps in the kernel, and while FEX suspends a thread it spins in FEX's
own code until the suspend is done. So `SuspendThread` returned while the thread went on running, the x86 context
of another thread could not be read (`NtQueryInformationThread(ThreadWow64Context)` answered for the calling thread
only), and `TerminateThread` of another thread was refused (`STATUS_NOT_SUPPORTED`). The Lab's lifecycle probe
showed it on every runtime up to v0.2.4: the counter of a suspended thread went on, the terminated thread was still
running after 5 s. Programs need both: Mono and .NET collectors suspend every thread and read its registers, and
games end hung workers with `TerminateThread`.

Horizon lets a process pause one of its threads (`svcSetThreadActivity`) and read its registers
(`svcGetThreadContext3`), but not change them: there is no `svcSetThreadContext` outside debugging.

## Decision

- Every Wine thread carries a stop request in the runtime's thread registry (patch `0031`): suspend, end, and the
  exit code.
- `SuspendThread` of a started thread without a doorbell pauses it with `svcSetThreadActivity` when it is in PE code
  — FEX, the translated program, Windows DLLs — and keeps its registers for `NtGetContextThread`. In the runtime's
  own code a thread can hold a lock the caller needs next, so there it is let go with a suspend request and stops
  itself at its next system call, sleep or server wait; reading its registers waits for that and tries the pause
  again every 20 ms (`0032`, `0033`, `0034`). A sleep waits on the thread's request, so a request wakes it at once.
- The x86 context of another thread is read and written in its WoW64 CPU area (`0033`); FEX does both while it
  suspends a thread.
- `TerminateThread` sets an end request and lets a paused thread go: it ends itself at its next system call, its
  server waits return `STATUS_THREAD_IS_TERMINATING`, and any exception it takes ends it. FEX frees a thread's state
  before the thread is terminated, so the thread's next step into FEX faults and ends it there.

## Rejected

- Ending a paused thread from outside: Horizon cannot change another thread's registers, and a thread kept paused
  for good would keep its place among the 96 threads and never signal its handle.
- Pausing a thread anywhere: in the runtime it may hold newlib's malloc lock, `virtual_mutex` or the server's lock.
