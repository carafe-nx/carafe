# Threads: the 96-thread limit and what the runtime spends of it

Started for issue #31: a 32-bit Direct3D 9 game dies when it tries to create a thread past `threads=96/96`.
Autorun was read at `636913ca` (the pinned submodule) unless a line says otherwise.

## The limit

- Atmosphère's process manager gives every application 96 threads: `BaseApplicationThreads = 96` in
  `stratosphere/pm/source/impl/pm_spec.cpp` (master, commit not recorded). The runtime only reads this value back
  (`dlls/ntdll/unix/horizon.c:19739-19746`).
- A refusal is `0x10801`: module Kernel, description 132, `LimitReached`.
- `BoostApplicationThreadResourceLimit` raises the limit to 256 (`ExtraApplicationThreads = 256 - 96`), but only when
  the kernel has spare threads (`g_extra_threads_available` in the same file). Autorun `9b02ed2` (2026-10-06, after the
  pinned commit) calls it at start-up and forces `ShouldIncreaseThreadResourceLimit` to `true` in its own Mesosphere
  patch. Whether the call changes anything on a stock Atmosphère is not verified.
- Eden gives a process 800 threads (`src/core/hle/kernel/k_resource_limit.cpp:167`). Carafe Lab's Eden patch 0050
  sets it to 96, so Lab meets the limit where the console does.

## What a game gets

Measured in Carafe Lab with `tools/probes/threads.c` (32-bit, DXVK, FEX, Horizon sync), run `2026-10-19_04-44-53`:

- The probe could hold 35 threads of its own. With a Direct3D 9 device open it could hold 30: DXVK keeps 5.
- Every Windows thread costs two Horizon threads. When the program ends with 35 Windows threads, the runtime ends 70
  (`[QUIT] … ending 70 threads`). The second one is the in-process server's thread for that client:
  `horizon_server_handle_new_thread` (`horizon.c:11527`) starts `horizon_server_thread` for each new thread
  (`horizon.c:11583`), and that thread serves the client's requests until its pipe closes (`horizon.c:15947`).
  Autorun's current `HEAD` still says "Each client connection is served by its own pthread".
- The rest belongs to the runtime itself; see the map below.
- With `Thread synchronization = Horizon` waits, events, mutexes and semaphores run on the calling thread
  (`horizon_server_sync_call`, `horizon.c:15892-15945`), yet the server thread is still created for every client.
- No leak was found: 300 threads that ended with their handles left open did not lower the ceiling; 300 threads
  closed right after creation lowered it for a moment (35 → 31), until the next creation reaped them.

## Map of the threads

Every Horizon thread of the probe, from Carafe Lab's thread stacks with the runtime's symbols (run
`2026-10-19_17-04-06`, 32-bit, DXVK, FEX, Horizon sync). At rest, with a Direct3D 9 device open, the process has 34;
with every thread the probe could take, 96:

| Role | At rest | At the limit | Where |
| --- | --- | --- | --- |
| Windows threads | 5 | 36 | the program's main thread and DXVK's; then the probe's |
| Server thread of each Windows thread | 5 | 36 | `horizon_server_thread`, `dlls/ntdll/unix/horizon.c:15947` |
| TLS page holders | 12–13 | 12–13 | `tls_placeholder`, `horizon-wine/source/runtime.c:3084` |
| Mesa queues | 5 | 5 | `util_queue_thread_func`, in the prebuilt Mesa; which queues is not verified |
| Compositor | 1 | 1 | `presenter_thread`, `horizon-wine/source/compositor.c:319` |
| Input | 1 | 1 | `wine_nx_input_thread`, `dlls/win32u/winnx_drv.c:90`, polls with `usleep` |
| Shared user data clock | 1 | 1 | `usd_clock_thread`, `dlls/ntdll/unix/virtual.c:5549`, polls with `usleep` |
| Stall watch | 1 | 1 | `stall_watch`, `horizon-wine/source/runtime.c:451`, polls with `svcSleepThread` |
| Log flusher | 1 | 1 | `log_flusher`, `horizon-wine/source/runtime.c:276`, polls with `svcSleepThread` |
| Save commit | 1 | 1 | `commitLoop`, Carafe's `runtime/overlay/carafe_overlay.c:1204` |
| In the exception handler | 1 | 1 | sleeps in `__libnx_exception_handler`; whose it is is not verified |

- The runtime keeps 24–25 threads of its own, the server one per Windows thread, so a program gets
  `(96 − 25) / 2 ≈ 35` threads.
- The TLS page holders exist only so that the kernel's thread-local pages stay out of a 32-bit program's memory: the
  kernel puts a page at random when no page has a free slot, and the runtime keeps one thread per page so its pages
  are never given back (`runtime.c:3062-3076`). They hold 13 of the 96 places for the whole run.
- Five threads wake up on a timer only to look at something: input, the shared data clock, the stall watch, the log
  flusher and the save commit.
- With `Thread synchronization = Standard` the probe also held 31 threads with a device open, the same as with Horizon
  (run `2026-10-19_16-48-10`): both modes start a server thread for every Windows thread.

## What the server threads serve

Counted by `runtime/patches/autorun/0019-horizon-thread-budget-and-server-requests.patch`, which logs a `[SERVED]`
line every 30 s (`[SERVER]` is taken by the profiler's round trips, `horizon-wine/source/thread_profile.c:850`). Last
lines of run `2026-10-19_17-32-48`, about two minutes of each game in Lab's standard scenario:

- OpenTTD: 75 058 requests through the client threads, 13 815 served on the calling thread. 62 160 of them were
  `GET_HANDLE_FD`, the file descriptor behind a handle that every file read and write asks for; then
  `SET_QUEUE_MASK` 2 103, `GET_OBJECT_INFO` 1 879, `CLOSE_HANDLE` 1 427. The 34 client threads that ended had served
  51 to 500 requests each.
- Need for Speed Underground: 14 057 through the client threads, 22 994 on the calling thread; `GET_HANDLE_FD` 3 743,
  `CLOSE_HANDLE` 1 621, `GET_OBJECT_INFO` 1 170, `OPEN_KEY` 886, `CREATE_FILE` 881. 17 client threads alive.
- Time inside the handlers was 2–3 s per launch; the switches to the client thread and back are not in it.
- Every thread asks the server something in its first moments, so starting a client thread only when one is needed
  would save little: a few shared threads or serving on the calling thread is what frees the places.
- `GET_HANDLE_FD` alone is 83 % of OpenTTD's requests and only looks a handle up. Served on the calling thread like
  the waits are, it would also save two thread switches per file read.

The five Mesa queues are probably the two shader disk caches (`disk$`), Gallium's driver thread (`gdrv`), glthread
and the shader compiler: these are the queue names in the runtime's binary; which thread is which is not verified.

## A stall after many short threads

After its churn phases (900 threads that start and end at once), the probe stands still when it opens its
Direct3D 9 device: 4 runs of 4 since `2026-10-19_17-57-44`, with this runtime and with the one before patch 0019. The
main thread waits in `NtWaitForAlertByThreadId` under `ntdll.dll+0x6d700`; new threads started for the device spin in
`RtlEnterCriticalSection` (`ntdll.dll+0x6d024`, called from `RtlLookupFunctionEntry+0x30`) or wait next to it, one at
90 % of a core (run `2026-10-19_18-25-22`, `backtraces.txt`). It looks like the start-up stall of OpenTTD in Carafe's
roadmap; whether it is the same lock is not verified. Without the churn phases the device opens at once.

## Thread-local page holders give their place

- `0024` and `0025`: at the limit, a refused thread creation first ends one of the 13 page holders and tries again.
  At the limit every page is full, so the holder's page stays and the new thread takes its slot; a page the kernel
  places anew is still checked against the program's reservations (`tls_retries` stayed 0).
- The probe then holds 77 threads with a Direct3D 9 device open instead of 64; with the device it is 82 Windows
  threads of 96.
- `Threads probe`, three runs each in turn: 2 of 3 passed with the pool alone, 3 of 3 with the holders giving way
  (runtimes `ab5f90b47f04` and `30e5890520b5`). The failures were DXVK refused while the probe's threads were still
  ending: in a fresh Eden they end slowly, and the 5 s wait runs out.
- A later run stood still for good: the creating thread waited for the holder to end inside libnx's
  `threadCreate`, and the holder never ended. The creator now waits for it at most 50 ms; a holder that ends later is
  closed on the next refusal or when the program ends, and only then counts as a free place.

## Why many threads stall

- The release of 64 held threads varies from 0.3 s to more than 60 s in Eden. It is slow in a fresh Eden and fast
  when the same Eden ran the probe before: Eden translates the runtime's code as it first runs. Whether the console
  stalls too is not verified.
- Patch 0005 wakes an endless alert wait every 250 ms. Waits of that length would point at lost wakes, so the
  runtime was built once with 25 ms instead: the release took 19.4 s and 0.3 s, as before. Not lost wakes.
- While the release stalls, the process uses 0.2–0.4 of a core (`--diag profiler`): the threads wait in turn.
- The probe's `locks` mode makes 4, 16 and 48 threads take one critical section 2000 times each. Every entry is
  counted, none is lost, and with 48 threads it took 4.5 s, with 290 waits over 100 ms and one of 1.2 s (run
  `2026-10-19_21-31-28`). A thread woken for the section joins the back of its core's queue at priority 59, behind
  the 10 ms slices of the threads there, and the section is taken by another one meanwhile.
- Tried and dropped: a thread sleeping for an alert at 58 and going back to 59 once woken, as decision 25 does for
  libnx mutexes. 48 threads took about half as long (1733–3767 ms against 3556–4844 ms, runs in turn; the longest
  wait 136–859 ms against up to 1242 ms), but `Threads probe` passed 0 of 3 with it on top of the holders giving
  way, against 3 of 3 without it: DXVK was refused while the probe's threads were still ending. A program
  surviving its peak comes first.

## A place comes free when the thread is closed

- The kernel gives a thread's place back when the thread object is destroyed, after its last handle is closed, not
  when it exits (`KThread::Exit` releases only a hint, `KThread::PostDestroy` the place; Eden's kernel,
  `src/core/hle/kernel/k_thread.cpp`, which follows Mesosphère). A thread creation at the limit that sees an exited
  thread still open waits for it in the kernel, up to 10 s (`KResourceLimit::Reserve`).
- Wine's threads are closed by the next thread to end (`exit_thread` joins the one before it), so the last one to end
  holds its place; the runtime's server threads close each other the same way.
- The runtime's thread creation holds `mapping_mutex` around `svcCreateThread`, so such a kernel wait stopped every
  memory change in the process with it, the ending threads' own among them.
- `0026` and `0027`: at the limit a creation no longer calls the kernel. It joins the Wine thread and the server
  threads that have ended, without waiting for any that has not, then takes a page holder's place or waits.

## Why the threads end late in Eden

The runtime's stall watch took the threads' stacks during a refused wait (run `2026-10-20_00-54-52`, three reports
5 s apart, all alike): the 41 probe threads let go and the main thread wait in `RtlWaitOnAddress` of the arm64
`ntdll.dll`, the way FEX's own locks wait, and three or four run in FEX and queue for newlib's `malloc` lock. FEX
translates the code the ending threads run, and in Eden that translated code is translated again. No thread ended
for more than 10 s, so a creation waiting 5 s was refused in 2 runs of 3 (runtime with `0026`, `0027`) and 1 of 3
before them. Whether the console is as slow is not verified.

- The threads that run are in FEX's compiler (`FEXCore::IR::IREmitter`, `ConstrainedRAPass`, `ARMEmitter`) and its
  allocator (`FEXCore::Allocator::malloc`, `aligned_alloc`), from `libwow64fex.dll`'s symbols.
- `0029` makes the stall report name a Windows thread's native callers from its stack. With it (run
  `2026-10-20_03-02-13`) the waiting threads stand at two of FEX's locks, both taken for every block FEX compiles:
  - about 22 at the one mutex of the decoder's buffer pool (`LookupCache::FindBlock` →
    `Decoder::SetupDecodeInstructionsAtEntry` → `PoolBufferWithTimedRetirement::ReownOrClaimBuffer`, and
    `GenerateIR` → `DelayedDisownBuffer`, both into `RtlAcquireSRWLockExclusive`);
  - about 13 at the code invalidation lock (`CompileBlock` → `WritePriorityMutex::lock_shared`). It lets a writer go
    first, and the writer is `InvalidationTracker::InvalidateAlignedInterval`, called from `wow64`'s
    `NtFreeVirtualMemory` as an ending thread frees its memory. Each such free waits for every block being
    compiled and holds up every new one.
- 41 threads ending at once therefore compile their way out one lock at a time, and every free they make stops all
  compiling. FEX comes prebuilt from Autorun's DLL repository (`horizon-dlls`); changing this means building FEX.
- With the wait at 20 s (`0028`) `Threads probe` passed 3 of 4 and 2 of 4. Of the failures one was a refusal after
  20 s; in the others the probe was still making its second device when Lab looked: it took 89 s once without any
  thread waiting for a place (run `2026-10-20_02-13-56`). The process stood still in FEX, not at the limit.
- The same picture, threads waiting in the native `RtlWaitOnAddress` while a few compile, is OpenTTD's start-up
  stall in Carafe's roadmap.

## The crash

- When the limit is reached, `NtCreateThreadEx` fails and Windows code gets an error. DXVK's own thread wrapper
  throws `std::system_error("Failed to create thread")` and nothing catches it: the string is in the bundled
  `dxvk/d3d9.dll`, built against libc++. The process ends with exit code 3.
- The probe ended with the same lines as the issue's log: `[THREAD] create failed rc=0x10801 threads=96/96` and
  `libc++abi: terminating due to uncaught exception of type std::__1::system_error`.
- The `[THREAD] create failed … 96/96` line at the very start of every log is not this problem: it is the runtime
  filling the limit with TLS placeholder threads on purpose (`horizon-wine/source/runtime.c:3076-3109`).

## Waiting for a place

`runtime/patches/autorun/0020` makes a refused thread creation wait up to 5 s for a place while the program runs.
Carafe Lab's `Threads probe` takes every thread, asks for a second Direct3D 9 device and lets half of its threads end
2 s later, as a burst of loading threads would:

- without the wait (runtime `c42a97510abc`, run `2026-10-19_19-03-49`) DXVK ends the probe with exit code 3, as in
  issue #31;
- with it (runtime `90edc254b356`, runs `2026-10-19_18-45-00` and `2026-10-19_18-55-23`) the device is made once the
  threads end and the probe finishes.

With no thread ending (`Threads probe hard`) DXVK still ends the probe: the wait cannot make places that never come
free.

## Fewer server threads

- `0021` and `0022` serve `GET_HANDLE_FD` on the calling thread. OpenTTD's requests to the client threads fell from
  75 058 to 8 943, and the direct ones rose from 13 815 to 45 391 (run `2026-10-19_19-40-43`).
- `0023` replaces the client threads with a pool when Horizon sync is on: a client's request pipe tells the pool it
  holds data, a pool thread serves one request and moves on, and a pool thread that waits inside a request counts as
  blocked so another one starts. With it the probe holds 69 threads at start (35 before) and 63–64 with a Direct3D 9
  device open (30–31), with 2 pool threads and one bootstrap server thread at the limit (run `2026-10-19_20-16-25`).
- OpenTTD's second launch had used 66 of the 96 at start-up before the pool (18 Windows threads, 21 server threads):
  a real game, not only the probe, comes close to the limit.
- When 64 threads end at once the process stands still for 10–20 s: new threads wait on a critical section in ntdll
  at start, each ending thread joins the one that ended before it (`exit_thread` → `pthread_join`), and each writes a
  `[LIFECYCLE]` line. More threads make this stall longer; it is the stall of the churn phases above. The probe now
  times it: 64 threads let go at once took 19 293 ms to end, one of them more than 10 s (run `2026-10-19_20-45-54`,
  `Threads probe census`).
- Carafe Lab's `Threads probe` passes with the pool: 64 threads with a device open, and DXVK makes its second device
  once half of them end (run `2026-10-19_20-42-25`). OpenTTD, Need for Speed Underground and Heroes of Might and
  Magic IV pass Lab's standard scenario with it (run `2026-10-19_20-27-00`).
