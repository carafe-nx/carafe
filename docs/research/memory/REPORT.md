# Memory on the Switch

Initial research snapshot: 2026-11-05. The original study made no implementation changes.
Six studies read the runtime's code, Horizon's and Mesosphère's sources, Windows and Wine
semantics, other systems, the graphics stack and 355 console and 998 lab logs. Each one is in `findings/` with its
sources; this report puts them together and orders the work. Numbers marked "estimate" are to be replaced by a
measurement.

- [findings/horizon-memory-model.md](findings/horizon-memory-model.md) — what Horizon gives an application
- [findings/runtime-consumers.md](findings/runtime-consumers.md) — who takes memory in the runtime today
- [findings/log-evidence.md](findings/log-evidence.md) — what the logs show, game by game
- [findings/windows-semantics.md](findings/windows-semantics.md) — what Windows does and what games rely on
- [findings/os-techniques.md](findings/os-techniques.md) — macOS, iOS, Linux, Android and what fits a 4-core A57
- [findings/graphics-memory.md](findings/graphics-memory.md) — DXVK, NVK and what games are told about video memory

## Updates after the study

As of 2026-11-10, [decision 67](../../decisions/0067-anonymous-memory-can-use-separate-backing-blocks.md)
records separate backing pieces for anonymous sections and smaller pieces for ordinary anonymous mappings.
The original contiguous-allocation restriction described below is therefore historical. This does not establish
that Cat Mail Co or Fields of Mistria now works; those cases were not rerun in this validation.

[Decision 68](../../decisions/0068-section-placement-and-host-address-space.md) records section destinations claimed
before anchors are placed, preservation of kernel error classes, removal of the forced Win32 4 GiB limit, and the
corrected host-layout selection. Fixed x86 images at or above 128 MiB can use a 39-bit host. The guest's own
large-address-aware flag still determines its Windows allocation limit. Double Agent works on the console in the
reported test, with no section failures and at least 808 MiB of heap free in the sampled log; the Lab campaign run
instead ends in a native Eden crash. The decision keeps both results and the remaining unmap rejection separate.

The findings and measurements below retain the original study's baseline. Proposed demand paging, file-page
reclaim, compression and SD swap were not implemented by these fixes. The findings files are dated snapshots,
not an assertion that every described limitation remains in the current runtime.

Console-log citations in the findings use `console-logs/<capture>/...`; Lab captures use
`lab-runs/<run>/...`. These identify captured evidence from the original study, not repository directories.

## What a game has

- A 4 GB Switch gives an application 3285 MB, Erista and Mariko alike; nothing in the NSP or in Atmosphère raises it on
  a stock console. Only a console with 8 GB fitted gets more (6964 MB with Atmosphère 1.11.0+ and its memory mode).
- Carafe's loader takes one heap of about 3168 MB and keeps 96 MB out of it (`HEAP_RESERVE`, no reason in the source).
  About 96 MB more of the process's limit is used before the runtime starts; by what is not known.
- Everything comes from that one heap: the runtime itself (87.6 MB, 44.6 MB of it static tables, among them Box64's
  11 MB and FFmpeg's 16 MB), Wine, the game's memory, copies of the files it maps, stacks, FEX's translated code and the
  GPU's memory, which on the Tegra X1 is the same RAM.
- Freed memory goes back to the runtime's allocators, never to the kernel. Horizon has no demand paging, no file-backed
  mappings, no swap and no overcommit: a mapped page is a physical page.

In the original sample, 39-bit hosts had about 3082 MB of heap and 32-bit hosts about 1849 MB. These are host-layout
measurements, not limits determined solely by the guest's instruction set. An empty 64-bit program already held
230–300 MB.

## Why a game needs more here than on Windows

The main finding: the runtime pays for memory Windows does not pay for, in five places.

1. **Commit is paid at once.** Windows charges a commit only against the commit limit and gives a page when it is
   first touched; a game that commits 1 GB and touches 300 MB uses 300 MB. The runtime takes and zeroes every committed
   page at once, in 64 KB chunks. Only anonymous mappings of 256 MB or more are filled on first touch.
2. **Mapped files are full copies.** On Windows a mapped file is page cache: shared between views, read on first touch,
   dropped under pressure and read again. Here every view of a file, every image and every NLS file is read whole into
   the heap when it is mapped, and two views of one file are two copies. Unity and Unreal map their asset files.
3. **Freeing part of a block frees nothing.** Memory is handed out in blocks of up to 16 MB. A partial decommit keeps
   the whole block until its last piece goes, and a recommit of the same pages takes new memory. Sections without a
   file are allocated whole when created, even with `SEC_RESERVE`.
4. **Nothing is ever reclaimed.** `MEM_RESET`, `DiscardVirtualMemory`, working-set trimming and memory priorities do
   nothing; `OfferVirtualMemory` does not exist; no low-memory signal reaches a game.
5. **Graphics keep copies and take what they are told they may.** Games are told of video memory that does not exist
   apart from RAM:
   - a 64-bit D3D11 game sees 2391 MB of dedicated video memory plus 2391 MB shared; a 32-bit one 1536 + 1536 MB;
   - D3D9 managed textures keep a full CPU copy for life (estimate 100–400 MB in a D3D9 game);
   - DXVK keeps up to 512 MB of empty chunks for 20 s on unified memory and never evicts;
   - Mesa keeps up to 128 MB of freed buffers and trims them only when a Vulkan allocation fails.

How much each of these costs in a real game is not measured: the logs explain only 600–780 MB of Cat Mail Co's
3030 MB.

## What fails and how

In the original log sample, two games run out: Cat Mail Co (console, 3030 MB used) and Fields of Mistria (lab, 10 of 14
launches). Megabonk came within 141 MB. No 32-bit game in that sample came near its limit in normal play, and no steady
leak was observed: the captured NFS sessions stay flat for hours. Later Double Agent failures and their fixes are
recorded in decisions 67 and 68 above.

Cat Mail Co grows by about 800 MB from the main menu to "new game". A 4 MB commit inside a reservation fails, and:

- it must be one contiguous block; there is no fallback to smaller pieces inside a reservation;
- the runtime loses the out-of-memory error on the way and returns `STATUS_ACCESS_DENIED` (`virtual.c:2529`), after it
  has already marked the pages committed (`virtual.c:2502`);
- the game writes to them, faults with `c0000005`, Unity's crash handler loads, and the thread spins forever;
- "52 MB free" is the sum of two allocators that cannot lend to each other (small allocations grow up, aligned ones
  down), so the failure may be fragmentation as much as exhaustion. The logs cannot tell which.

Fields of Mistria fails a 2 MB allocation that reaches Horizon as a 2.06 MB commit; every block over 2 MB is rounded
up to a multiple of 2 MB, so it takes 4 MB.

The captured failures do not show the games recovering from the failed allocations. They motivate reducing the
runtime's memory costs before an allocation is refused; they do not establish how every Unity or Unreal game handles OOM.

## What a good memory manager would be

The aim is the one macOS and iOS show: a page costs RAM only while it is needed, clean pages are dropped and read
again, cold dirty pages are compressed before anything goes to storage, and the system asks for memory back before it
refuses. Horizon gives none of this, but it gives the parts to build it from:

- `svcUnmapPhysicalMemory` / `svcMapPhysicalMemory` free and take pages at 4 KB in the alias region, which Carafe has
  (64 GB) and does not use; this is how memory can really be given back;
- data and instruction aborts reach the runtime's exception handler and can be resumed; Autorun already uses this for
  lazy commit, so pages can be filled, decompressed or read on first touch;
- memory the kernel or the GPU reads (IPC and SVC buffers, transfer memory, nvmap buffers) never faults to us and must
  stay resident, and only one thread per process can be in an exception at a time.

With these the runtime can keep each guest page in one of a few states: reserved, committed and untouched (no RAM),
resident, clean and backed by a file (droppable), compressed, and on storage. The A57 has no access bits, so cold pages
are found by protecting pages and seeing which are touched.

Precedents on the same chip: NVIDIA turns zram on by default on the Jetson Nano (Tegra X1), and Switchdeck offers zram
and SD swap to games on the Switch under Linux. No emulator pages guest memory itself.

## Vectors, in order

Each step is useful alone; later ones build on earlier ones.

### 0. See where the memory goes

Nothing below can be chosen well without it. In the runtime's log, every 10 s and at the first failure:

- bytes per consumer: guest commits by kind (private, file view, image, section), large blocks, FEX code, GPU
  (device-mapped bytes, Mesa's buffer cache, DXVK's chunks), Wine's heaps, the runtime's own;
- the largest free block of each allocator, to tell "out of memory" from "out of contiguous memory";
  `wine_nx_native_heap_stats` already computes it;
- committed versus touched pages, and the bytes lost to 2 MB rounding;
- a snapshot at the moment of the first failure, not up to 10 s later.

Then Cat Mail Co, Fields of Mistria and Megabonk in the lab show which of the causes above is the big one.

### 1. Pay for memory as Windows does

The biggest gain for the least cost (estimate; to be confirmed by step 0):

- every commit filled on first touch, not only maps of 256 MB or more; all-zero pages not kept;
- a partial decommit frees exactly its pages, through the alias region;
- sections without a file allocated as their pages are used;
- a commit inside a reservation may be made of several pieces;
- mapped files and images as clean pages: read on first touch, shared between views, dropped under pressure and read
  again. No storage writes, so no SD wear. The largest piece of work here, and possibly the largest gain for engines
  that map their assets.

### 2. Graphics that fit unified memory

- the D3D9 managed-texture copy dropped after upload and read back from the GPU on lock (Experimental, per game);
- static buffers mapped directly, without staging;
- Mesa's buffer cache at 32 MB instead of 128 MB;
- DXVK keeping fewer empty chunks for a shorter time, and smaller D3D11 upload staging during loads;
- an honest video-memory figure where a game sizes its textures by it (per game; the rows exist).

### 3. Ask for memory back before refusing

- on a failed commit: trim the runtime's own caches (Mesa's buffers, DXVK's empty chunks, FEX's old code, empty pool
  arenas) and try once more;
- if it still fails, return the status Windows returns, so the error screen says the game ran out of memory;
- the low- and high-memory signals of `CreateMemoryResourceNotification` set from the real state;
- a reserve kept for the overlay, so the settings menu opens when the game is out of memory.

### 4. Compressed memory

Cold pages compressed in RAM with LZ4, after a fast check that rejects pages that do not compress (block-compressed
textures barely do). Estimate: 0.5–0.7 GB gained if a third of the heap is cold. A fault costs about 15–25 µs
(estimate), so faults per frame must be capped to keep frame times; pages the GPU or the kernel read are never touched.

### 5. Swap on the SD card

Only behind the compressed tier: large sequential compressed writes into a file created once at full size, a slot kept
until its page is written again, a write budget, never the internal storage, off by default as an Experimental option.
Through Horizon's `fs` a request costs about 0.83 ms and runs at 37–48 MB/s, so 4 KB paging would give about 5 MB/s.
Autorun's swap experiment (`WINE_NX_SWAP_POC`) is off in Carafe, needs Autorun's launcher and pages only private
memory; it is a starting point, not a solution.

### Side fixes

- The failure status: the out-of-memory error kept instead of `STATUS_ACCESS_DENIED`, and pages not marked committed
  when the commit fails.
- Two wrong hints in the settings: `dxvk.maxMemoryBudget` does not limit DXVK's memory on this GPU, and
  `d3d9.textureMemory` saves address space, not memory; the memory error's suggestion relies on the first.
- The 96 MB `HEAP_RESERVE` and the 96 MB used before the runtime starts.
- Static tables of components a game does not use (Box64's, FFmpeg's) in the runtime's own 87.6 MB.
- The lab's `peakMemoryMb` is the whole Eden container, not the game's heap.

## Measurements on the console

Four short checks decide the costs of steps 4 and 5, and one decides step 1's form:

- `page-census` — in a real game, the share of all-zero pages and the LZ4 ratio of the rest;
- `codec-a57` — LZ4 and zstd speeds on the A57 (none published);
- `fault-a57` — one fault round trip, and one map/unmap pair at 64 KB and 2 MB;
- `sd-random` — reads and writes of 64 KB–2 MB through `fs` while the game reads its RomFS;
- whether 16 MB of system resource is enough for a pager over 3 GB in 64 KB units.

## Open questions

- What Cat Mail Co's 3030 MB really split into, and whether its failure is exhaustion or fragmentation.
- How Unity and Unreal size their pools from the memory and video memory they are told about.
- Which pool backs the NACP JIT memory, and whether any system service charges our limit.
- How much FEX's old and new code generations overlap.
