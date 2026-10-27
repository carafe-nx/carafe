# W11. Reading ahead from the SD card

## Answer

**The race.** NFSU2 does read while it races: 7–29 card requests every 10 s, 29–132 ms of card time and 1–4 MB from
its 118 MB `STREAML4R*.BUN` files. That is at most 1.3 % of wall time. The reads run on a thread other than the main
thread: the main thread's IPC is 0.0–0.2 % of its samples in those intervals. Read-ahead therefore gives about
**1.00× in frame rate**: under 0.2 % even if every one of the main thread's IPC samples were a read.

**Stutter** is the open part. A card request takes 4.1–4.6 ms in the race. If two to four of them a race interval
land on the main thread, each makes a 22 ms frame about 20 % longer. Whether they do needs a per-thread read log and
a frame-time maximum, which no current log has.

**Loading.** Read-ahead wins in loading, which spends 0.7–1.4 s of card time per load:

- 150–180 requests of about 300 KB at about 48 MB/s;
- a quarter to a third of it on the main thread.

A prefetch driven by a profile of earlier runs, on a free core, can hide up to that time: **0.7–1.4 s per load**. A
load takes far longer than that in CPU work, so the saving is a minor share of it. The duration of a load is not
measured.

**Where it goes.** The mechanism belongs in Carafe's own `sdmc:` overlay, under Autorun's cache, and needs no Autorun
change. One Autorun patch is worth adding, to `sd_cache.c`: the cache holds its only lock while a chunk is read from
the card.

## Evidence

### The read path

1. `NtReadFile` reads a regular file synchronously on the calling thread, overlapped I/O included: "async I/O
   doesn't make sense on regular files" (`external/autorun/dlls/ntdll/unix/file.c:6510-6530, 6595`, Autorun
   `636913ca`). Each read first asks the server for the descriptor (`GET_HANDLE_FD`). Carafe's `0021` and `0022`
   serve that on the calling thread.
2. newlib's `read` goes to the `sdmc:` devoptab, which is Autorun's SD cache (`hw/sd_cache.c:729-758`). The cache is
   installed as the first line of `main` (`hw/runtime.c:3503`), after Carafe's overlay has already replaced `sdmc:`
   in a constructor (`runtime/overlay/carafe_overlay.c:1657-1716`). So the cache's base device is Carafe's overlay.
3. The overlay sends `/switch/wine/…`, the game included, to the NSP's RomFS (`fsOpenDataStorageByCurrentProcess` +
   `romfsMountFromStorage`, `carafe_overlay.c:1499-1520`), or to save data, or to the card (`overlayRead`, `:830-846`,
   one lock per open file). RomFS reads become FS service requests on the installed NCA on the SD card. Whether the
   FS service decrypts on its own core is not verified.
4. In a Carafe NSP the runtime's `sd_reads`, `sd_ms` and `sd_mb` therefore count requests from the cache to the
   overlay, which for game files means RomFS requests, not raw card requests.

### The cache

`hw/sd_read_cache.h:1-44`, `hw/sd_cache.c`:

- **Chunks:** 128 KB, 8 per file, in a pool of 32–192 MB that grows with the heap the game leaves free
  (`sd_cache.c:103-157`). Reads of 64 KB or more bypass it.
- **Read-ahead is off.** Fills were tried growing from 16 KB, but on the card each request costs 0.83 ms before its
  transfer at 37–42 MB/s, and the smaller fills tripled the requests (`sd_read_cache.h:13-21`, `sd_cache.c:53-58`).
  Those numbers are for `fsFile` on the card, not for RomFS.
- **The lock.** `sd_cache_read_file` holds `sd_cache_mutex` around `sd_cache_read`, and that calls the fill: a card
  request (`sd_cache.c:421-430`). While one thread fills a small chunk, every other thread's cached read waits. A read
  of 64 KB or more goes straight to the base device without the lock (`:413-419`).
- **Writes.** Small writes are held back and sent every 200 ms (`sd_cache.c:309-315`, `hw/runtime.c:309`). Carafe's
  overlay commits save data from its own thread, priority 44, two seconds after the last write
  (`carafe_overlay.c:1459-1471, 1705-1713`). In the run below it committed twice, in 38.7 ms and 33.3 ms
  (`nfsu2-exp9/carafe-overlay.log`).

### What NFSU2 reads (console)

`D:\debug_switch\carafe\nfsu2-exp9\logs\autorun_runtime.log`, deltas of the `[PROGRESS]` counters per 10 s:

| Window | Phase (from `[FPS]` and `[D3D9 calls]`) | NtReadFile | read ms | card requests | card ms | card MB |
|---|---|---:|---:|---:|---:|---:|
| 0–5 s | runtime start-up | 4041 | 163 | 486 | 1653 | 92 |
| 25–65 s | menus, 60 fps | 10 | 9–19 | 1–3 | 4–8 | 0 |
| 65–75 s | load into the race | 177 | 1177 | 181 | 1167 | 56 |
| 75–85 s | end of load | 55 | 232 | 49 | 215 | 7 |
| 96–166 s | race, 35–54 fps | 15–35 | 43–144 | 7–29 | 29–132 | 1–4 |
| 176–186 s | load | 159 | 717 | 152 | 697 | 23 |
| 196–256 s | race | 9–13 | 12–24 | 1–3 | 4–15 | 0–1 |
| 256–276 s | load | 82 + 152 | 304 + 1076 | 75 + 153 | 293 + 1045 | 10 + 51 |

- **Every 10 s,** even in the menus, about 10 reads and 1 card request happen. These are probably the runtime's own;
  not verified.
- **Race requests** take 4.1–4.6 ms each and carry about 100–150 KB. Load requests take 4.6–6.8 ms and carry about
  300 KB, at about 48 MB/s.
- **The streamed data.** The game's `TRACKS` folder holds `STREAML4RA.BUN` (118 MB), `STREAML4RD.BUN` (121 MB) and
  `STREAML4RB.BUN` (28 MB), next to the smaller `L4R*.BUN` files (local copy in `D:\Game\Need for Speed Underground
  2`). The race reads fit the streaming of world sections. Which file each read comes from is not logged.
- **Who reads.** The main thread's share of `svc 0x21` (IPC) per interval, from the `[PROF] 4w svc` lines:
  - 0.0–0.2 % in the race intervals (lines 1090, 1140, 1240, 1290, 1342, 1394);
  - 1.4 % at 75–85 s (line 1008);
  - 2.3–2.5 % in loads (lines 1502, 1962).

  `+0x1906024` is the common IPC stub, which `dxvk-cs` reaches too, so these samples include IPC other than FS. In
  the race at most 10–20 ms per 10 s of IPC are on the main thread, against 29–132 ms of card time in all. Most of
  the race reads run elsewhere, on a thread below the sampler's 3 % threshold. In loads the main thread holds
  230–250 ms of the 700–1045 ms.
- **What the race frames cost.** The `[FPS]` lines give only 10-second averages (35.4–53.6 in the race, lines 1075,
  1125, 1175, 1226, 1275, 1327). No frame-time maximum is logged, so a 4–5 ms hitch cannot be seen.
- **Mapped files.** File views: Horizon has no page cache, so a view of a file is read whole when it is made. Autorun
  `4c9841d0` (after the pin) reads large read-only views 512 KB at a time on first touch, behind `lazy-file-views`.
  NFSU2 makes 132 `create_mapping` requests at start-up (line 795) and unmaps views in its loads (`unmap_view` 458,
  line 1494). Whether any of them is a large data file, rather than an image, is not verified.

### The arithmetic

- **fps in the race:** at most 0.2 % of the main thread waits on IPC → ≤ 1.002×.
- **Stutter:** if 2–4 race requests in 10 s land on the main thread, each adds 4.1–4.6 ms to one 18–28 ms frame.
  That is 2–4 frames in 450–540 that run 15–25 % long. Not verified.
- **Load:** card time per load is 0.70 s at 176–186 s, 1.38 s at 65–85 s and 1.34 s at 256–276 s. A prefetch that
  has the data in memory when the game asks hides the waiting part. That is all of it on a reader thread, and the
  main thread's 0.23–0.25 s wherever it reads. So the gain is 0.7–1.4 s per load at most. It depends on the prefetch
  being right: the same files in the same order as the profile.
- **Start-up:** 1.65 s of card time in the first 5 s. A boot-set prefetch on a helper thread, while Wine starts,
  could hide part of it. How much the boot work overlaps is not measured.

## Design

**In Carafe's overlay** (`runtime/overlay`, compiled into the runtime, no Autorun change):

- **Read log.** Path, offset, length, thread, time since the last frame and duration, for each read, into a per-game
  profile kept in save data between runs (W12). Off in release builds except as a short summary.
- **Profile-guided prefetch.** When a read matches the start of a recorded sequence, a helper thread reads the next
  recorded ranges into a buffer pool:
  - the thread is native, priority 60, on a core the main thread is not on;
  - the pool is capped by the same rule as the cache's: a third of the heap left above 512 MB;
  - the game's read then becomes a copy. A miss falls back to the normal read.
- **Sequential read-ahead** for files read in order in large requests (the streaming files): keep the next one or two
  requests' worth ready. This only shortens the streaming thread's waits; it matters if the game shows pop-in.
- **Whole-file residency** for the active track's stream file (118 MB), only if stutter is measured and the heap has
  the room. In the race 720–850 MB of the native heap were free (`heap_free_mb`).

**In Autorun** (one patch to `hw/sd_cache.c`): fill a chunk outside `sd_cache_mutex`, with the line marked as
filling, so that another thread's cached read does not wait behind a card request. Only if contention is measured.

**For games that map their archives,** Autorun's `lazy-file-views` (when Carafe rebases) is the read-on-demand piece.
The prefetch above then applies to the chunks those views read.

## What this means for the orchestrator

- **Storage is not a frame-rate resource in NFSU2.** The orchestrator's job there is to move reads, and their
  waiting, off the main thread and ahead of time.
- **In loads** it gives the prefetch helper a free core (0 or 2) at a priority below the game's busy threads. It
  stops prefetching when the heap's free share falls.
- **Per-game data.** The read profile is part of the per-game profile kept between runs. The first run records it;
  later runs use it.
- **Core 3 is not needed:** the work is mostly waiting on the FS service.

## Needs the console or the lab

| Measurement | How | Decision it switches |
|---|---|---|
| Reads per thread | Overlay: per TID, count, bytes and total ms of reads, into `[PROGRESS]`. Lab for correctness, console race and loads | Whether the main thread reads in the race (stutter) or only in loads |
| Worst frame | Overlay or `[FPS]`: maximum and 99th-percentile frame time per report, and how many reads overlapped the worst frame | Whether race reads cause hitches worth a read-ahead |
| RomFS request cost | `[A57]`-style probe at start-up: reads of 16 KB, 128 KB and 1 MB from a RomFS file, cold, ms each | Prefetch chunk size; whether the cache's 0.83 ms + 42 MB/s model holds for RomFS |
| Load duration | Overlay: time from the first read of a load burst to the first frame with draws again | How large 0.7–1.4 s is against a load |
| Cache lock waits | Patch `sd_cache.c` (lab first): time spent waiting for `sd_cache_mutex`, per thread | Whether the fill-outside-the-lock patch is worth carrying |
| Read profile replay | Lab: record a run's reads, replay them as prefetch on the next run, count hits and misses | Whether profile-guided prefetch is accurate enough to ship |
