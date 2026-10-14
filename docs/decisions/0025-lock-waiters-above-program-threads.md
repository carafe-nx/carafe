# 25. A program thread waits for a lock above the program threads

Date: 2026-10-14

## Context

OpenTTD stood still for 20–110 s at start-up, in the emulator bench and on the console. Loading the title game starts
one `ottd:linkgraph` thread per link graph job, 17 at once, and they contend for the Win32 heap. Its critical section
waits through Wine's `RtlWaitOnAddress`, which guards its queues with a busy-wait `spin_lock`
(Autorun 636913ca, `dlls/ntdll/sync.c:847–851`). Every Wine thread is pinned to one core
(`dlls/ntdll/unix/horizon.c:3937`) at one priority, 59 (not verified). Horizon shares a core between threads of one
priority only by moving the running one to the back of the queue every 10 ms (Mesosphère and Eden, not verified).

The holder of the `spin_lock` allocates memory inside it, through FEX and newlib's one malloc lock. When the kernel
hands that lock to it, the holder joins the back of its core's queue behind the threads spinning on the `spin_lock`,
and runs again only after their 10 ms slices. Every thread needs hundreds of such handovers.

Bench, 6 launches in a row with the fix off and on in turn: off, 3 of 3 stood still (first frame at 80–111 s), 13–15
threads spinning in `ntdll.dll+0x53f72` and `+0x53a12` on a lock word of `0xffffffff`, the holder inside the
`spin_lock` waiting for the malloc lock; on, 0 of 3 (first frame at 24–54 s). The malloc lock waits that were counted
were short (at most 119 ms per wait), so where all the stalled time goes is not fully accounted for.

On the console, one NFSU race each way (215 s): after the first 50 s, 35.9 fps with the decision and 37.7 without, and
as many hitches longer than 250 ms (7 and 8). Most of the waits it raised (53 thousand of 61) came in the first 10 s;
in the race itself about 40 a second.

## Decision

- `runtime/overlay/carafe_lock_boost.c` wraps `svcArbitrateLock`: a thread at 59 waits for any libnx mutex at 58 and
  goes back to 59 as soon as it owns it. The kernel's priority inheritance runs the owner at 58, so a contended lock
  passes on at once instead of after the spinning threads' slices. Uncontended locks do not change.

## Rejected

- Making Wine's `spin_lock` give up the core: the i386 `ntdll.dll` comes prebuilt from Autorun, and in a test program
  in Eden it gave 78 iterations/s against 452 with this decision.
- Longer timeouts or FEX TSO: they do not change how the lock is handed over.
- Unpinning Wine threads: with more spinning threads than cores every core stays busy anyway.
