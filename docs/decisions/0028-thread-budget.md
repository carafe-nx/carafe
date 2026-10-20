# 28. The runtime keeps few threads of its own and makes new ones wait for a place

Date: 2026-10-19

## Context

A Switch application gets 96 threads (Atmosphère's process manager, `BaseApplicationThreads`). A program run through
Carafe could hold only 35 of them (issue #31, [research](../research/threads/REPORT.md)):

- every Windows thread costs two, because Autorun's in-process server starts a thread for every client
  (`dlls/ntdll/unix/horizon.c:11583` at `636913ca`);
- the runtime keeps 24–25 of its own: 12–13 that only hold thread-local pages out of a 32-bit program's memory, five
  Mesa queues, five threads that wake on a timer (input, the shared data clock, the stall watch, the log flusher, the
  save commit), the compositor and one more;
- when the limit is reached the kernel refuses at once, and DXVK ends the process on a refused thread.

Every thread asks the server something as soon as it starts, so starting a client's server thread only when it is
needed would save little. 83 % of OpenTTD's server requests are `GET_HANDLE_FD`, a lookup.

## Decision

- A thread that finds no place waits up to 20 s for one while the program runs, every 2 ms at first and every 20 ms
  after 100 ms, and says so in the log every 5 s (`runtime/patches/autorun/0020`, `0028`). Bursts of short loading
  threads are served late instead of failing. In Eden the threads that make room took more than 10 s to end.
- At the limit a creation does not call the kernel, which would wait up to 10 s for an ended thread nobody has closed
  while the runtime holds `mapping_mutex`. It closes the Wine thread and the server threads that have ended itself,
  without waiting for any that has not (`0026`, `0027`).
- The runtime reports its budget: `[BUDGET]` (threads against the limit, Windows threads, server threads, refusals,
  waits) and `[SERVED]` (what the client threads serve) every 30 s and on a refusal (`0019`). A creation that has
  waited 5 s writes where it was called from and has the stall watch write where the other threads stand, and the
  stall report names a Windows thread's native callers (`0028`, `0029`, `0030`). The waiting thread does not read
  the others itself: that kept it from seeing a place come free for up to a minute.
- Requests that only look something up or change it without waiting are served on the calling thread, as the waits
  already are with Horizon sync; `GET_HANDLE_FD` first (`0021`, `0022`).
- With Horizon sync the per-client server threads give way to a pool that takes requests as their pipes fill; a
  pool thread that waits inside a request counts as blocked and another one starts (`0023`). Standard sync keeps a
  thread per client, which also turns the pool off.
- At the limit a thread-local page holder gives its place to the new thread, since every page is full then
  (`0024`, `0025`).
- Every step comes as its own patch and passes Carafe Lab's threads checks and the games, A/B against the runtime
  before it, before the next one.

## Rejected

- Fewer DXVK compiler threads when places run short: DXVK starts them with the device, before a game is near the
  limit, and with about 80 places a game has room for them; fewer of them would only stutter more.
- A reserve of places for audio, the screen and the server: the runtime's threads start before the program, and the
  server pool keeps two spare workers from the first client on, so they never queue for a place behind the program.
- Waiting for a place with no time limit: a thread that asks for one inside the loader lock, as in a DLL's
  `DllMain`, would then stop the program for good, since ending threads take that lock too.
- Patching the kernel for more threads, as Autorun's own Mesosphère patch does: it is a change to the console's
  system that an NSP cannot carry and every Atmosphère update breaks.
- `BoostApplicationThreadResourceLimit` (to 256): it adds threads only when the kernel was booted with spare ones,
  which a stock Atmosphère is not known to do. Left for when Autorun's later changes are brought over and can be
  checked on a console.
- Running several Windows threads on one Horizon thread: the thread-local register is the kernel's, and Wine, FEX and
  Mesa keep their state behind it. Kept as a later step if the ones above are not enough.
- Starting a client's server thread lazily: every thread asks the server at start.
- Folding the runtime's timer threads into one. Each stands alone so that one stuck does not stop the others: the
  stall watch pauses other threads to read where they stand, the log flusher writes the log, the registry and the
  held-back card writes, the shared data clock ticks every millisecond, the input thread presents. Together they
  would save two or three places of 96.
- A thread waiting for an alert one priority above the program threads, as decision 25 does for libnx mutexes: it
  halved the time 48 threads took to share one critical section, but with it the program's threads ended too slowly
  for a refused thread to find a place, and `Threads probe` passed 0 of 3 against 3 of 3.
