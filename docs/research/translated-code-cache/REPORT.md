# Translated code kept between runs

A design agreed in discussion, not built and not measured. It covers 32-bit Windows games, which run on the console
under FEX (WoW64). FEX was read at FEX-2609 in the patched tree `runtime/build/fex-src`. Numbers marked "estimate" are
guesses to be replaced by the first measurement. NFSU is the first game to try it on; nothing here is specific to it.

## The idea

FEX translates x86 code to ARM while the game runs, block by block, the first time each block is reached, and forgets
everything at exit. Every run pays again for the same work, and the game stops for it whenever it reaches new code:
start-up, a new track, a menu, an effect.

The goal is to translate a game's code once and keep it: ahead of time where the code can be found, during play for the
rest, and to reuse the result on every later run. The player only drops a game folder into the app; nothing extra is
asked of them.

A full ahead-of-time recompilation of a game is the long-term form of the same idea. The parts that make it hard each
have an answer that fits this design:

- Code cannot be told from data by reading the file alone. FEX records the code that actually ran, and the PE file
  gives the rest of the starting points: the entry point, exports, base relocations (pointers to functions, virtual
  tables, jump tables), `.pdata` in 64-bit files, exception tables, and calls followed from all of them.
- Code missed ahead of time is still translated during play, as today, and kept from then on.
- Code unpacked by copy protection is seen once it runs.
- Code generated at run time (.NET/Mono, LuaJIT) is not kept; those engines are open source and can be swapped for
  native ARM builds, which is a separate task.
- DLLs loaded at another address are handled by FEX through the module's base relocations.

The game's own DLLs ship as machine code, the same as its `.exe`; there is no source to rebuild them from. The system
DLLs a 32-bit game uses, Wine's `syswow64` set and DXVK's `d3d9.dll`, are i386 and run translated too
([game speed](../game-speed/REPORT.md)). Their source is ours, so native builds behind a thunk are possible, the way
Windows on ARM ships hybrid system DLLs for x86 programs; that is the "native `d3d9.dll`" item of the game speed report
and stays separate from this cache.

## What FEX already has

Both mechanisms are off by default.

- **Disk cache** (`DiskCache`, `Config.json.in:147-202`, `FEXCore/include/FEXCore/Core/DiskCache.h`). Each translated
  block is written in the background to a cache file; on later runs a block found in the file is taken from it instead
  of being translated. The cache lives in a folder named after a hash of FEX's format version, its code-affecting
  settings and the host CPU features (`DiskCache.cpp:395-420`), so a cache made with other settings or on another CPU is
  never read. Extra read-only caches can be opened beside the writable one (`DiskCacheRODBNames`). The files use
  Mesa's Fossilize format and are only appended to; nothing is ever removed from them.
- **Ahead-of-time code cache** (`EnableCodeCachingWIP`, marked unfinished upstream). During play FEX writes a code map:
  which blocks of which module ran (`CodeCache.h`, `CodeMap`). `FEXOfflineCompiler` turns code maps into one cache file
  per module, which is attached when the module is loaded (`Source/Windows/Common/ImageTracker.cpp:154-263`). A module
  is identified by name, PE time stamp and image size (`ImageTracker.cpp:38`); base relocations are read from the PE
  (`ImageTracker.cpp:78-112`).
- The WoW64 module already wires the disk cache's file mapping (`Source/Windows/WOW64/Module.cpp:553`).
- The Horizon port loads cached code but turns off page-trap self-modifying-code detection with it
  (`horizon.patch:1534`). This has to be made safe before the cache is on for players.

## Where the cache lives

| Storage | Writable | Removed with the game | Use |
|---|---|---|---|
| RomFS, inside the NSP | no | yes | a cache made in advance |
| Device save data (256 MB, console memory) | yes | yes | game saves only |
| Account save data | yes | yes | no: a user picker at every start |
| Cache Storage | yes | yes | the cache the console builds |
| Temporary Storage | yes | cleared at exit | no |
| SD card folder `sdmc:/switch/carafe/<Title ID>/` | yes | no | no: left behind after the game is deleted |

Cache Storage is the application's own storage for data that can be lost without harm. It is a save-data container
kept by the system, on the console's memory or on the SD card (`Nintendo/save/`, encrypted), not tied to an account,
removed together with the game and shown in Data Management. Its size is declared in the NACP:
`CacheStorageSize`, `CacheStorageJournalSize`, `CacheStorageDataAndJournalSizeMax` (the size it may grow to) and
`CacheStorageIndexMax` (`external/hacbrewpack/nacp.h:54-57`); `app/crates/carafe-core/src/nacp.rs` sets none of them
today. It is a separate file system from the device save data, so a cache file open for writing the whole session does
not hold back the commit of game saves ([4](../../decisions/0004-sdmc-overlay.md),
[19](../../decisions/0019-commit-with-open-files.md)).

## Decisions so far

- **The console's cache goes to Cache Storage on the SD card.** It is removed with the game, does not take console
  memory, and does not block save commits. Device save data keeps game saves only. An SD folder was rejected because it
  stays on the card after the game is deleted.
- **RomFS holds a shared cache for Wine and DXVK**, made once when the runtime is built and packed into every NSP with
  the runtime. These DLLs are the same in every game, so every game starts without translating them.
- **A ceiling of 512 MB for now, as a setting later.** The ceiling is set in the app when packing and written to the
  NACP as the size Cache Storage may grow to; the working limit is changed in the console's settings menu, up to that
  ceiling.
- **Rarely used code goes first when the limit is reached.** For every block the cache records in how many runs it was
  needed and when last; when the cache is over the limit, it is rebuilt keeping the most used and most recent blocks.
  FEX's files are append-only, so this is Carafe's own code. Usage is recorded from the start; the rebuild is written
  when a game first gets close to the limit.
- **Preparation at the first start, as one step of the loading screen.** The app finds the game's code while packing
  (the PE starting points listed above) and writes the list into RomFS; at the first start on the console a loading
  step translates the list into the cache on three cores. Code that was not found is translated during play and kept.
  For now this is an experiment inside the existing loading screen; its own design, and a choice to let the player run
  it in the background, are agreed later.
- **After a Carafe update, the same preparation step runs again, and the old cache is deleted whole.** The same applies
  when a setting that changes translated code changes. A cache made by another FEX version or other settings is never
  read again, so keeping it only takes space.
- **A changed `.exe` or DLL (a game update, a mod) is translated again on its own.** Cache entries are checked by a hash
  of the module; nothing else is thrown away.
- **The first trial is on NFSU**, with the lab checking that a run with the cache draws the same frames as one without.

## Still to settle

- Whether a homebrew NSP can create and open Cache Storage through libnx, and on which storage the system places it.
  Fallback: read the cache from save data, write new blocks to a scratch file on the SD card, fold them in at exit.
- The journal limits how much can be written between commits; the cache has to be committed in portions, which matters
  most during preparation.
- Whether FEX's file mapping works on Horizon (`MapFile` through the Wine unix layer); FEX can read without mapping
  (`DiskCacheFileMapping=false`).
- Whether a cache made on the PC (in Eden) carries the same CPU-feature hash as the console; if not, the RomFS cache is
  ignored, harmlessly.
- Making the self-modifying-code check safe with cached code (`horizon.patch:1534`).
- Loading the cache in parts as code is reached (FEX has lazy loading), so it does not sit in RAM unused.
- A "reset cache" entry in the settings menu.
- Moving DXVK's shader cache from the SD folder to Cache Storage too, so it is removed with the game.

## Expectation

Estimates, before any measurement.

| What | Expected |
|---|---|
| Cache size | 20–60 MB for an older game like NFSU, 100–300 MB for a large modern one |
| Stutter from translation | almost gone from the second run; much less on the first run with preparation |
| Time to the main menu, second run | 20–50 % shorter; less in NFSU, where part of the start-up wait is lock handover between threads |
| Preparation step | 1–3 minutes once, finding 60–90 % of a game's code |
| Steady frame rate in a scene already running | 0–5 % from the cache alone |
| Frame rate with hot code rebuilt with heavier optimizations | +10–30 % |

The cache itself removes translation, not execution. Its larger value for speed is that translation paid once can be
slow and thorough: hot code can be rebuilt in the background with whole-function translation, direct jumps between
blocks, the x87 stack kept in registers, flags computed only where read, memory barriers dropped where only one thread
touches the memory, and instruction choice tuned for the Cortex-A57 (ARMv8.0, no FlagM, no LSE atomics), and the result
kept in the same cache.

## First trial

What is built for it:

- **The Experimental section of the settings** (the app and the console menu, the last tab) with a row "Translated
  code cache", off by default and shown only with FEX. It writes `FEX_DISKCACHE` into the program's settings.
- **`runtime/patches/autorun/0051`**: Autorun passes FEX only the options of its list (`fex_options.h`); the patch adds
  `FEX_DISKCACHE` and `FEX_DISKCACHEFILEMAPPING`, the second always off, as Horizon's unix side has no `MapFile`
  (`fex_unix.c` answers it `STATUS_NOT_SUPPORTED`) and FEX then reads the files.
- **The cache folder on the SD card.** FEX keeps the cache under `%LOCALAPPDATA%\fex-emu\DiskCache\<hash>\`
  (`Source/Common/Config.cpp`, `GetCacheDirectory`). The overlay routes `drive_c/users/*/AppData/Local/fex-emu` to
  `sdmc:/switch/carafe/<Title ID>/`, the same as DXVK's shader cache, so a file open the whole session does not hold
  back the commit of save data. Cache Storage replaces it later.
- **The profiler under FEX** (`runtime/patches/autorun/0050`): see below.
- **Carafe Lab keeps the caches between the launches of a run.** It used to move the whole SD folder of the game into
  the launch's logs, so every launch started without DXVK's and FEX's caches. Now the logs go and `drive_c/` stays;
  each launch folder gets `caches.txt` with the files and sizes. The first launch of a run still starts cold.

- **File locks on Horizon** (`runtime/patches/autorun/0052`). The first run with the cache on left an empty
  `RWCacheDB.foz` and no index: FEX takes a lock before it writes the file's header (`FOZFile::Open`,
  `DiskCache.cpp`) and gave up. Carafe Lab's files probe showed that `LockFileEx` and `UnlockFileEx` failed with
  error 1 (`ERROR_INVALID_FUNCTION`) on the SD card and in save data alike, while writes and reads worked: the
  runtime's in-process server (`dlls/ntdll/unix/horizon.c`) has no `lock_file` and `unlock_file` and answers
  "not implemented". The patch grants every lock, as Wine does on a file system without locks; the runtime is
  one process, so no other one can hold a lock. Any program that locks a file before writing it was affected,
  not only FEX.
- **No writer thread on Horizon** (`runtime/patches/fex/0004`). With locks granted, the next run ended 11 s into the
  loading screen with an access violation at `libwow64fex.dll+0x185a30`, on a thread whose x86 frames began at
  ntdll's `RtlUserThreadStart`. FEX hands each store to a writer thread (`WorkQueueThread`) that it starts with
  `NtCreateThreadEx` at an ARM64 function (`Source/Windows/Common/Threads.cpp`); on Horizon every new thread of a
  32-bit game starts as an x86 thread, so the writer ran FEX's own code as x86. On Horizon the cache now writes a
  block on the thread that compiled it, one store at a time.

In Eden, FEX's output is ARM code that Eden translates again for the PC on every launch, so a lab run shows only the
part of the gain that comes from FEX itself; the console has no second translation and should gain more.

## First results

In Carafe Lab (NFSU, one launch of each kind, time to the first frame): without the cache 62 s and 116 s, with it 88 s
cold and 54 s warm. The cache held 35 MB after the first launch with a race and 41 MB after the second. Eden's spread
between launches is larger than the difference, so the lab does not show a gain.

On the console (NFSU2, one launch each, about a minute of play):

| Launch | First frame | SD reads in the first 5 s | In-game frame rate, 10 s averages |
|---|---|---|---|
| Runtime before the cache work, no cache | 12 s | 493, 0.16 s | 37–47 |
| Cache on, first time | 22 s | 504, plus 263 writes | 34–53 |
| Cache on, second time | 22 s | 1320, 4.6 s | 36–51 |

The cache held 68 MB, its index 2.4 MB. All three runs ended cleanly, without stalls or errors.

- The frame rate in the game does not change, as expected: the cache removes translation, not execution. The main
  thread was busy 95–99 % in all three runs.
- Start-up got 10 s longer and the warm cache did not win it back. On the second start the time goes into reading
  the cache: without file mapping FEX reads every blob with its own call through Wine and the overlay. On the first
  start every store is written on the game's thread.
- Whether lookups hit is unknown: nothing counts hits and misses. A start with no gain at all suggests either few hits
  or reads that cost as much as translation.
- Stutter, the gain the cache is for, is not visible in 10 s averages.

Next: hit and miss counters with the time spent reading and translating; the whole cache read into memory at start
instead of one read per blob; stores batched instead of one per block on the game's thread; a frame-time log on the
console with the number of long frames per interval. Then one console check of the same route with the cache on and
off, switched in the Experimental section of the console menu.

## The profiler under FEX

The runtime's sampler (`thread_profile.c`) pauses a busy thread every 2 ms and reads its registers. Under Box64 it
asked Box64 for the x86 address; under FEX all translated code fell into "pe" as bare addresses. FEX keeps its thread
state pointer in x28, and the state's first word (`CPUState::InlineJITBlockHeader`) is the header of the block being
run; the header's tail gives the block's size, its first guest address and its RIP entries, the same data FEX's
`RestoreRIPFromHostPC` reads. When the sampled PC lies inside that block, the profiler walks the entries to the guest
address, and the x86 frames start from FEX's ESP and EBP (x8 and x9). The offsets were checked against FEX's headers
with `static_assert`s and the entry decoder against FEX's own encoder on two million random pairs.

The `[PROF]` lines then have `x86 by module` for FEX games too, and a new `pe by module` line splits the ARM64 PE time:
`libwow64fex.dll` is FEX itself (dispatcher, compiler), `jit` is FEX's code buffer outside any block.

## Order

1. Measure in the lab: FEX's disk cache on, NFSU started twice; time to the main menu, frames over the stutter
   threshold, average frame rate, cache size.
2. Cache Storage: NACP fields, creating and opening it, committing in portions, the FEX cache folder pointed at it.
3. Preparation: finding code in the PE files when packing, the loading step that translates it.
4. The shared Wine and DXVK cache in RomFS.
5. Usage records, the limit and its two settings; the rebuild when a game nears the limit.
6. One check on the console.

FEX does not accept AI-written code, so changes to it stay in `runtime/patches/fex`.
