# W10. Translation ahead of the game on a free core

## Answer

Yes. FEX can already compile on any thread that has its own FEX thread state, and the result is shared: all threads
use one code buffer and one guest-to-host map, compiling takes only a shared lock, and a block another thread
compiled is found by the game thread on its next lookup miss and linked on first use. FEX's own ahead-of-time
generator on Linux does exactly this, one compiler thread per core, and finds the next blocks from `call` targets and
each block's branches out of it. What is missing on Horizon is a thread to run it on: a 32-bit game's new threads all
start as x86, so the helper has to be a guest thread that enters FEX through a custom entry point, or Autorun has to
learn to start a native thread with a Wine TEB. Code memory needs no permission change: Autorun maps it twice, RW
and RX, and the existing flush already reaches the other cores. The gain is in stutter and start-up, not in the race's
average frame rate: in the race the main thread spends under 0.5 % in FEX's C++ (compile included) and "link-compile"
shows 0.0 %. Compile time per block on the A57 is not measured; NFSU2 compiles about 15000 multiblocks in a long
session (distinct keys in its disk cache), so at 0.1–0.5 ms each that is 1.5–7.5 s of one core, most of it at start,
the first track and new menus. Moving it to core 3 or a free core removes those seconds from whichever thread hit the
new code, and removes compile hitches inside frames once the next blocks are known ahead: from the previous run's list
(W12) on every run after the first, from the PE's starting points and each block's exits on the first. With the disk
cache on, the same helper also loads cached blocks ahead and writes new ones, so neither the cache's reads nor its
stores fall on the game's thread, which today makes start-up 10 s longer.

## Evidence

FEX at `395b132` in the patched tree `runtime/build/fex-src`; Autorun at `636913ca`.

### Where compiling happens now

- The dispatcher's L1 miss calls `ContextImpl::CompileBlock` on the thread that reached the code
  (`Dispatcher.cpp:320-353`); a direct exit not yet linked calls `ExitFunctionLink`, which looks the block up and
  compiles it on a miss (`JIT.cpp:531-556`).
- `CompileBlock` (`Core.cpp:875-1050`): takes `CodeInvalidationMutex` shared, with signals deferred (`:894`); returns
  at once if the block exists (`:898`); tries the disk cache (`:904-941`); compiles (`:946`, `CompileCode`
  `:816-873`); marks the guest pages as code for SMC tracking (`:1005-1016`); stores the block to the disk cache and
  the code map (`:1018-1034`); inserts every entry point into the lookup cache (`:1036-1039`).
- Everything the compiler works on is per thread: decoder, IR emitter, pass manager, backend
  (`InitializeCompiler`, `Core.cpp:408-428`). Code is first emitted into a per-thread temporary buffer and copied into
  the shared code buffer at the end "to minimise lock contention" (`JIT.cpp:897-905`, `1122-1146`).
- Shared state and its locks:
  - the code buffer: one per process, shared by all threads (`SharedCodeBufferManager.cpp`); on Horizon its storage
    is Autorun's `CodeStorage`, allocated with a compare-and-swap, with a mutex only when a new chunk is added
    (`horizon-wine/fex/code_storage.h`); 256 MB at most for a 32-bit game (`fex/cache_policy.h:18-20`);
  - the guest-to-host map ("L3"), one per code buffer, behind a write-priority reader-writer mutex
    (`LookupCache.h:49-163`): writers are `AddBlockMapping`, `AddBlockExecutableRange`, block links; readers are the
    C++ lookups after an L1 miss (`LookupCache.h:182-235`). L2 is off on Horizon (`Config.json.in:277-283`), so a
    miss goes straight to this map;
  - `CodeInvalidationMutex`: shared while compiling or linking, unique only while code is invalidated (SMC, unmapped
    memory, patch 0001's narrowed case);
  - a race between two compilers of the same block is already handled: `CompileCode` looks again before handing the
    IR to the backend and drops its own result (`Core.cpp:836-852`).
- Each thread has its own L1 table; a block another thread compiled reaches the game thread through one L1 miss and
  one map lookup (`LookupCache.h:218-225`), after which it sits in its L1. In the race that lookup is
  `GuestToHostMap::BlockList` find at 0.1 % of the main thread (`libwow64fex.dll+0x1a800` in `SPEED2_profiler.log`,
  symbolised with `tools/pesyms.js` against the current `libwow64fex.dll`, which may differ slightly from the run's
  build).

### FEX already compiles on many threads ahead of time

`Source/Tools/FEXInterpreter/AOT/AOTGenerator.cpp:60-170`: scans a section for `call rel32` targets (and `endbr64`),
queues them, starts one thread per CPU, each with `CTX->CreateThread()` and `ConfigureAOTGen` so the frontend reports
the branch targets it did not follow (`Frontend.cpp:1215-1218`), compiles each with `CompileRIP`
(`Context.cpp:22-24`, which is `CompileBlock` on that thread's frame), and queues the new targets. `CodeSizeValidation`
and `FEXOfflineCompiler` call the same entry. So compiling on a thread that is not running the guest is a supported
use of FEXCore.

### What stands in the way on Horizon

- **No native helper thread.** FEX starts its disk-cache writer with `NtCreateThreadEx` at an ARM64 function; on
  Horizon every new thread of a 32-bit game starts as x86, so the writer ran FEX's code as x86 and the game crashed
  (Carafe patch `0004`, translated-code-cache report). A plain Horizon thread created by the runtime would have no
  Wine TEB, and the compile path makes NT calls: `MarkGuestExecutableRange` reprotects pages with
  `NtProtectVirtualMemory` (`WOW64/Module.cpp:500-502`, `InvalidationTracker.cpp:270, 353, 421`), and `PreCompile`
  processes WoW64 cross-process items (`WOW64/Module.cpp:520-522`). Two ways out:
  1. a guest thread created normally (it gets a TEB and a FEX thread state), whose start address is a guest address
     registered with `AddCustomIREntrypoint` (`Core.cpp:1109-1124`) and lowered to a `Thunk` op that calls the
     worker loop natively (`BranchOps.cpp:334-362`); the worker then calls `CompileRIP` with its own thread state.
     This stays inside FEX's existing mechanisms;
  2. an Autorun change so a native start address starts a native thread with a TEB, as Windows WoW64 does. That is a
     change to Autorun's thread start (D1).
- **Code memory.** Autorun backs FEX's code with `svcCreateCodeMemory` and maps it twice, RW for the owner and RX for
  the slave (`horizon-wine/source/fex_jit.c:52-98`); FEX writes through the RW alias (`WritableAddress`) and never
  changes permissions, so a helper on another core writes the same way. Every instruction-cache flush is a unix call
  that takes one mutex and runs `armDCacheFlush` on the RW alias and `armICacheInvalidate` on the RX range
  (`fex_jit.c:330-350`, `CodeEmitter/Buffer.h:80-86`). Cache maintenance by address to the point of unification is
  broadcast to the inner-shareable domain on ARMv8, so a block written on core 3 is visible to the game's core once
  the flush has returned and the block's address has been published through the map's lock. FEX already relies on
  this whenever one game thread runs code another one compiled. Linking patches one `b`/`bl` at a time
  (`JIT.cpp:577-593`), an instruction swap ARMv8 allows while other cores run the code.
- **The disk cache turns off page-trap SMC detection** (`horizon.patch:1530-1535`): a helper that loads cached code
  inherits that limit until it is made safe (translated-code-cache report).

### How much compile time there is

- Race (exp9, `SPEED2_profiler.log:1597-1600`): `pe` 0.7 % of the main thread, `libwow64fex.dll` 0.4 %,
  `link-compile` 0.0 %. Compiling does not show in the race's averages.
- Loading (`SPEED2_profiler.log:762-765`): `libwow64fex.dll` 15 %, but its top buckets are
  `WowSyscallHandler::HandleSyscallImpl` and `HandleSuspendInterrupt` (the `QueryPerformanceCounter` spin), not the
  compiler.
- Block count: the 180 MB disk cache of an earlier NFSU2 session (`nfsu2-cache-db`, read with `tools/foz-check.js`)
  holds 36868 valid index records for 15515 distinct blocks (the file predates `0004`'s append fix, so records
  repeat). The cache of the October run held 68 MB.
- Compile time per block on the A57: not measured. FEX records it per thread (`AccumulatedJITTime`,
  `AccumulatedJITCount`, `SHMStats.h:59-81`, accumulated in `CompileBlock` at `Core.cpp:889`), but only with
  `FEX_PROFILESTATS`, whose shared-memory slots come from a unix call (`Windows/Common/SHMStats.cpp:28-37`); Autorun's
  FEX unix table answers most calls with `STATUS_NOT_SUPPORTED` (`fex_unix.c:131-135`), so the stats are off on the
  console (not verified which index the slot allocator uses). On Horizon the counter is `CNTPCT_EL0`, 19.2 MHz ticks,
  not cycles (`SHMStats.h:18-30`).
- First frame on the console: 12 s without the cache, 22 s with it on the first and the second start; on the second,
  4.6 s of the first 5 s went to 1320 SD reads of cache blobs, one call each through Wine and the overlay
  (translated-code-cache report).

### Finding the next blocks

| Source | What it gives | Cost and risk |
|---|---|---|
| Previous runs (W12): the code map FEX writes (`CodeMapWriter::AppendBlock`, `Core.cpp:1031-1033`) or the disk cache's index | every block that ran, by module and offset; with the profile, in order of heat and of first use | exact; nothing on the first run |
| PE starting points (translated-code-cache report): entry point, exports, base relocations into `.text` (function pointers, vtables, jump tables), `call rel32` targets as in `AOTGenerator.cpp:60-80` | most functions of a module | false targets in data; compiles code that never runs (memory, time) |
| Each compiled block's exits (`ExternalBranches`, `Frontend.cpp:1215-1218`; call targets and jump-table targets found by W7-b3) | the likely next blocks, one step ahead | cheap, follows the game; does not see indirect targets |
| The lookup misses of the game thread | blocks reached right now | only as a last resort; it is the stutter itself |

Pages: only blocks in non-writable image sections are safe to compile ahead without changing SMC behaviour, because
`MarkGuestExecutableRange` only reprotects RWX intervals (`WOW64/Module.cpp:500-502`); code in writable memory
(unpackers, generated code) is left to the game's thread, as now.

### What the others do

Box64 keeps translated code on disk (`DYNACACHE`) and builds big blocks (`BIGBLOCK`)
([USAGE.md](https://github.com/ptitSeb/box64/blob/main/docs/USAGE.md)); FEX's `FEXOfflineCompiler` builds per-module
caches from code maps ahead of time (translated-code-cache report); Rosetta 2 translates whole binaries ahead and
keeps them, with a JIT for code generated at run time (Apple's "About the Rosetta translation environment"; page not
readable through the fetch tool here).

## What this means for the orchestrator

- A **compile service** on a free core: one worker thread (a guest thread entering FEX through a custom entry point),
  placed on core 3 at a priority below the graphics workers when Autorun's forwarder is there, otherwise on core 0 or
  2, whichever is idle (`[THREADS]` shows both mostly free in the race). It keeps a queue of guest addresses with
  priorities:
  1. blocks of the previous run's list for the module just loaded, hottest and earliest first (W12);
  2. the exits of each block just compiled, by the game thread or by the worker;
  3. PE starting points, only in a "prepare" step and only in non-writable `.text`.
- With the disk cache on, the worker takes the cache's reads and stores too: it loads a module's cached blocks when
  the module is mapped and writes new blocks in batches, so the game's thread never waits on the SD card for code.
- The worker yields whenever the game thread is short of a core: the orchestrator watches main-thread busy share and
  frame time, and pauses the queue during frames that run long for reasons other than compiling.
- The same worker runs W7's second tier (retranslating hot blocks with the profile) when the queue is empty, and
  publishes the result as in W12.
- Without the forwarder (three cores), the worker runs on whichever of cores 0 and 2 carries less, at a low
  priority; the gain is the same, only the place differs.
- Expected effect: race average fps unchanged (compiling is under 0.5 % there); fewer long frames on new code;
  start-up shorter by the compile time of what the game thread compiles now (unknown, 1.5–7.5 s by the estimate above,
  part of it overlapped with loading waits already) plus the cache's read and store time on the game's thread (up to
  10 s measured).

## Needs the console or the lab

| # | Measurement | How | Decision it switches |
|---|---|---|---|
| W10-M1 | Compile time and count per thread | FEX `ProfileStats` on Horizon with its slots in process memory instead of a unix-call SHM; `[PROF]` prints JIT time, JIT count, lookup misses, map lock wait and disk-cache hits, misses and lookup time per thread per report | If the main thread's JIT time is under 1 % at start and in the race, W10 is only about the cache's I/O; if seconds, the worker comes first |
| W10-M2 | Long frames and their cause | frame-time log on the console: frames over 33 ms per interval, with the main thread's JIT time inside the same frame | How much of the stutter translation-ahead can remove |
| W10-M3 | Start-up split | time to first frame and to the race, with JIT time and disk-cache time on the main thread and on others | The order of the worker's queue (previous-run list first or PE scan first) |
| W10-M4 | Worker correctness | Carafe Lab: NFSU and NFSU2 with the worker on, same frames, no new faults; FEX's map and invalidation paths under two compilers at once | Before the console |
| W10-M5 | Contention | the map's read and write lock wait times from W10-M1 with the worker on | Batching the worker's map inserts |
| W10-M6 | Cross-core publication | lab and console: a stress test where the worker compiles blocks the game thread enters at once; any wrong-code fault ends it | Whether an ISB on the game's side after an L1 miss is needed (not verified that it is not) |

Not verified: compile time per block on the A57; whether a custom-entry guest thread can host FEX's compile loop
without other side effects (TLS, signal deferral); that Autorun's suspend path never stops the worker in the middle
of a map update.
