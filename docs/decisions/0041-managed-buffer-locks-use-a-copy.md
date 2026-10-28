# 41. Locks of managed buffers use a copy

Date: 2026-10-28

Builds on [decision 37](0037-d3d9-front-end-inside-dxvk.md).

## Context

With the front end on its own thread, every lock that is not queued first waits until the queue has run out. In
NFSU2's race the only locks that still wait are of managed buffers, without flags: 100–240 per 600 frames, 1.2–2.1 ms
of the game thread per frame. DXVK itself never waits for the GPU on such a lock: a managed buffer is written in host
memory and uploaded at the next draw that uses it. The wait is only there because queued draws must still see the old
contents.

## Decision

- **The thread that queues keeps a copy of the buffer**, made from the buffer's own memory after one wait, the second
  time the buffer is locked: buffers filled once while loading are not copied. A lock hands out the copy; a read-only lock costs nothing more; the unlock queues the changed range, and the worker
  writes it into the buffer in order with the draws, as it does for the staged discard locks.
- **Any lock outside the queue drops the copy**: from another thread, or one larger than 256 KB. The next queued lock
  takes the copy again. Buffers above 4 MB are not copied.
- **The effect worker hands uploads on** to the DXVK worker instead of locking the buffer itself, so the wait does not
  move onto the busiest thread.
- **An Experimental row of its own**, `d3d9.managedBufferCopies`, off by default, that works only with the front end
  row.

## Rejected

- **Staging every lock without flags.** A default-pool buffer may be read back after the GPU wrote it; only managed
  buffers are written by the CPU alone.
