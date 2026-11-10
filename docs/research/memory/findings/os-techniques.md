# How modern OSes make limited RAM go further, and what fits Carafe

Research snapshot: 2026-11-05. For subsequent runtime changes and validation, see the
[report](../REPORT.md#updates-after-the-study).

Scope: macOS/iOS (XNU), Linux/Android, Windows, game consoles, emulators and Windows-on-phone stacks; measured
compression ratios and speeds; user-space paging designs. Read-only research: web sources and a short look at the
Autorun tree (`carafe/external/autorun`, the submodule's current commit). Nothing was built or run.

Labels: **[F]** is a fact with a source. **[H]** is a hypothesis or an estimate of mine, with the reasoning. Numbers
marked [H] must be measured on the console before they drive a decision.

## Answer

macOS is economical mainly for three reasons. Clean pages (code, mapped files) can be dropped and re-read at no
cost. Dirty pages that go cold are compressed in RAM before anything reaches the disk. Apps and system caches give
memory back when the kernel signals pressure. iOS works the same way with no swap at all, which makes it the closest
model for a console. Of the three, Horizon gives us none for free: it has no demand paging, no file-backed mappings
and no hardware access bits (the A57 is ARMv8.0, and hardware Access-flag updates arrived in ARMv8.1). Everything has
to be rebuilt in user space on top of the exception handler and our own map/unmap. That is feasible. Autorun already
has the pieces: demand commit for reservations of 256 MB and more, and an off-by-default SD paging proof of concept
(`WINE_NX_SWAP_POC`). The ranking at the end says what to build on them, in order of gain against cost.

1. Stop paying for pages that hold nothing: zero or never-touched pages.
2. Make the Windows "app contract" report a budget the game can live in.
3. Drop clean pages and re-read them from the file.
4. Add a compressed-RAM tier (LZ4) in front of any SD tier.
5. Only then put truly cold data on SD, under a write budget.

The fault path costs microseconds and SD costs about a millisecond per random read. The 4-core A57 can compress
roughly 100 MB/s per core [H]. Together these mean the system must be proactive and run in the background, and it
must cap how many faults the game's main thread takes per frame.

## 1. macOS / XNU

### 1.1 The compressor

- [F] Apple introduced compressed memory in OS X 10.9: it "uses the dictionary-based WKdm algorithm", "automatically
  compresses the least recently used items in memory, reducing the size of items by more than 50 percent", and
  compresses or decompresses "a page of memory in just a few millionths of a second". It reduces swap-file reads and
  writes. ([OS X Mavericks Core Technologies Overview, Oct 2013][mav]; summarised by [AppleInsider][ai-mav])
- [F] WKdm (Wilson, Kaplan, Smaragdakis, USENIX 1999) is a word-oriented compressor with a direct-mapped 16-word
  dictionary. It is built for in-memory data (pointers, small integers, zeros) and beats LZO on small blocks in speed;
  the paper reports paging-cost savings of 20–80 % on the hardware of the time ([paper][wk99]).
- [F] Current XNU has three codec modes: `CMODE_WK`, `CMODE_LZ4` and `CMODE_HYB`. On ARM the default is hybrid. If
  WKdm's output exceeds `lz4_threshold` (2048 bytes for 4 KB pages, 12288 for 16 KB pages), the page is tried with
  LZ4 ([vm_compressor_algorithms.c][xnu-alg]). XNU ships hand-written arm64 WKdm (`WKdmCompress_4k.s`,
  `WKdmDecompress_4k.s`, `_16k` variants) ([xnu arm64 tree][xnu-arm64]).
- [F] Compressed pages live in a compressor pool of segments (`c_segment`). When the pool fills, segments are written
  to swap files under `/private/var/vm/swapfile*`; on iOS the pool stays in RAM ([memory-pressure overview][macint],
  [vm_compressor_backing_store.h][xnu-bs]).
- [F] An app's footprint on iOS is defined as dirty plus compressed memory, and clean memory "doesn't really count"
  ([WWDC 2018 session 416 transcript][wwdc416]).
- [F] There is a warning about the contract. Freeing cached objects during a memory warning can raise memory use for
  a while, because compressed pages have to be decompressed before they can be freed. The example is a dictionary of
  three pages compressed to one ([WWDC 2018 session 416][wwdc416]).

Practical savings and CPU cost: Apple claims more than a 50 % reduction per compressed page [F]. I found no published
system-wide figure for the CPU share. The design points that matter for us follow.

- The compressor works on pages already chosen as LRU victims, so only cold pages pay.
- It rejects or switches codec on a poor ratio: the 2048-byte threshold [F].
- Disk is only the overflow for the compressed pool.

### 1.2 Purgeable memory

- [F] `vm_purgable_control` puts a region into volatile, nonvolatile or empty states. Volatile memory can be
  reclaimed by the kernel with no write-back. `NSPurgeableData` and `NSCache` build on it. The client must call
  `beginContentAccess`, and a result of NO means the data was purged and has to be regenerated
  ([Caching and Purgeable Memory][purg]).
- [F] Windows has the same idea. `OfferVirtualMemory` makes pages inaccessible, takes them out of the working set and
  never writes them to the page file. `ReclaimVirtualMemory` gets them back intact or as garbage, and
  `DiscardVirtualMemory` discards them at once ([OfferVirtualMemory][offer], [ReclaimVirtualMemory][reclaim],
  [Raymond Chen's survey of MEM_RESET/Offer/Discard][oldnew]).

### 1.3 Pressure levels, notifications, jetsam

- [F] XNU has the levels Normal, Warning ("relax caching") and Critical ("drop all caches, expect latencies").
  Warning starts at 0.5× available memory and Critical at 0.3–0.4× (embedded and macOS). Processes subscribe with
  `DISPATCH_SOURCE_TYPE_MEMORYPRESSURE`. There are also per-process notifications at 80 % of the process's limit
  ([xnu doc/vm/memorystatus_notify.md][xnu-notify]).
- [F] Jetsam keeps a pool of fully reclaimable memory (5–15 % of maximum memory) and kills processes by priority
  band, lowest first. A process that exceeds its high-water mark is killed outright. Before a kill, memorystatus sends
  `NOTE_VM_PRESSURE`, which UIKit turns into `didReceiveMemoryWarning`. iOS has no swap
  ([xnu memorystatus_notify][xnu-notify], [Levin, "No pressure, Mon!"][levin]).
- [F] iOS apps can read `os_proc_available_memory()`. The `increased-memory-limit` entitlement raises the per-app
  limit on some devices ([entitlement overview][ios-ent]).
- [F] iOS's freezer writes frozen apps' compressed memory to NAND under a daily budget:
  `FREEZE_DAILY_MB_MAX_DEFAULT 1024`, with "degraded perf." when less than 25 % of the budget is left
  ([kern_memorystatus_freeze.h][xnu-freeze]). Apple rations flash writes from paging explicitly.

### 1.4 Clean vs dirty, file-backed pages

- [F] Clean memory covers mapped files, images and framework `DATA_CONST`. The kernel can drop it and page it in
  again from the file, so it does not count toward the footprint. Dirty memory is anything the app has written
  ([WWDC 2018 session 416][wwdc416]).
- This is the cheapest technique in any OS: no compression, no write-back, no flash wear. Its cost is a read on the
  next touch.

### 1.5 App Nap

- [F] App Nap throttles timers, lowers I/O priority and lowers CPU priority for apps that are doing no user-visible
  work. Apple quotes up to 23 % less power ([Energy Efficiency Guide: App Nap][appnap]). It saves energy, not memory.
  It has no counterpart for a single foreground game.

## 2. iOS as the console analogue

- [F] iOS has no swap. It relies on compression, jetsam and the app contract: footprint limits, warnings, and
  `EXC_RESOURCE` when a limit is exceeded ([WWDC 2018 session 416][wwdc416], [Levin][levin]).
- What transfers to Carafe: the footprint idea (dirty plus compressed is what counts, clean is free), a hard budget
  with an early warning, and cooperative caches. A Windows game does not take part in this contract, but the layers
  we own (Wine, DXVK, Mesa, FEX) can [H].
- What does not transfer: killing. We run one process, the game, and killing it is a crash. The only analogue is a
  clean error screen when memory really runs out (decision 0032).

## 3. Linux and Android

### 3.1 zram and zswap

- [F] zram is a compressed RAM block device used as swap. It stores same-filled pages (including zero pages) without
  an allocation and keeps incompressible ("huge") pages uncompressed. It can recompress idle pages with a stronger
  secondary algorithm. It can write idle or incompressible pages to a backing device, and `writeback_limit` exists
  because "if there are lots of write IO with flash device, potentially, it has flash wearout problem"
  ([zram doc][zram]).
- [F] zswap is a compressed cache in front of a real swap device. It detects same-value pages and writes LRU pages
  back to disk when the pool reaches `max_pool_percent`. `accept_threshold_percent` is a hysteresis against
  thrashing. It "trades CPU cycles for potentially reduced swap I/O" and reduces "life-shortening writes" on flash
  ([zswap doc][zswap]).
- [F] Google runs zswap as a far-memory tier across its fleet. The median compression ratio is 3× (range 2–6×), cold
  memory is defined by a 120 s threshold, and the target promotion rate is 0.2 % of the working set per minute. CPU
  overhead is 0.001 % for compression and 0.005 % for decompression of fleet cycles, and the tier delivers 4–5 % DRAM
  TCO savings ([Lagar-Cavilla et al., ASPLOS 2019][gfm]; [summary][gfm-sum]).
- [F] Android's low-RAM guide says "compression ratios in the 30–50% range are usually observed" for zram. That is,
  compressed size is 30–50 % of the original, or 2–3.3× ([Android low-RAM guide][alowram]).
- [F] The Jetson Nano uses the same SoC family as the Switch (Tegra X1, 4× A57, 4 GB). JetPack ships with zram on by
  default: four devices of about 494 MB, 2 GB in total ([JetsonHacks][jhacks]). NVIDIA considered compressed swap
  worth having on this exact CPU.
- [F] Switchroot's L4T Ubuntu on the Switch ships 1 GB of zram. Switchdeck runs Windows games through Proton with
  Box64 or FEX on the Switch. It offers `SD_ZRAM` (an extra 1–2 GB of zram) "for games close to the RAM limit" and
  `SD_SWAP` (4 GB of SD swap) "required for some games that run out of RAM". It also unloads Steam helpers to "free
  up over 1GB" ([Switchdeck README][swdeck]; [Switchroot changelog][swroot]). This is the nearest published precedent
  to Carafe: same hardware, same kind of workload, Linux doing the paging.

### 3.2 Swap on slow media

- [F] ChromeOS uses zram rather than disk swap and limits zram writeback to protect eMMC
  ([zram writeback throttle patch][zram-wb]).
- [F] SD Association A1 cards guarantee 1500 random-read IOPS, 500 random-write IOPS and 10 MB/s sustained
  sequential. A2 cards guarantee 4000 and 2000 IOPS, but need Command Queue and Cache support in the host
  ([SDA Application Performance Class][sda]).
- [F] Random 4 KB writes cause a write amplification factor of 2–5 on SD cards. Consumer TLC NAND is rated at around
  3000 P/E cycles, and QLC often below 1000 ([ATP: dashcam endurance][atp], [HT Semi: microSD life][htsemi]).

### 3.3 MGLRU, DAMON, KSM, THP, lmkd

- [F] MGLRU groups pages into generations, using accessed bits found by page-table walks. `min_ttl_ms=1000`
  "usually eliminates intolerable janks due to thrashing" ([MGLRU doc][mglru]). Google reported 18 % fewer low-memory
  kills on Android, and on ChromeOS 51 % less kswapd CPU and 96 % fewer tab discards ([Phoronix][mglru-ph]).
- [F] DAMON samples one page per region per sampling interval (5 ms by default, aggregated every 100 ms), using
  PTE Accessed bits. It splits and merges regions adaptively, so its overhead is bounded by the number of regions,
  not by memory size. DAMOS adds proactive reclaim ([DAMON design][damon]).
- [F] KSM merges identical pages in regions marked with `MADV_MERGEABLE`. ksmd scans 100 pages and then sleeps
  20 ms by default. Merging can be unprofitable: the kernel says to drop the policy if
  `rmap_items/merging_pages > 64`. `use_zero_pages` merges empty pages with the zero page ([KSM doc][ksm]). Android
  warns that KSM costs CPU and battery ([Android low-RAM guide][alowram]).
- THP saves no memory; it trades memory and paging granularity for TLB reach. Carafe's page-size situation is in
  [memory-pages.md](../../orchestrator/findings/memory-pages.md). One consequence for paging: changing the protection
  of one 4 KB page splits a 2 MB block (`SeparatePages`, same document).
- [F] Android's lmkd uses PSI stall time instead of vmpressure, which had "numerous false positives". Its thresholds
  include `psi_partial_stall_ms` 70 / 200 ms and `swap_free_low_percentage` 20 / 10 % for high-end / low-RAM devices
  ([lmkd doc][lmkd]).

### 3.4 userfaultfd: user-space paging on Linux

- [F] userfaultfd lets user space resolve missing-page faults (`UFFDIO_COPY`, `UFFDIO_ZEROPAGE`), write-protect faults
  (`UFFDIO_WRITEPROTECT`), and minor faults (`UFFDIO_CONTINUE`). Its write-protect mode is described as "equivalent
  to (but faster than) using mprotect and a SIGSEGV signal handler". It exists because VMAs cannot track faults at
  page granularity over large address spaces ([userfaultfd doc][uffd]).
- [F] Measured on an i7-8665U at 1.9 GHz: mprotect plus SIGSEGV costs 1.92 µs per fault (0.74 µs to deliver,
  0.36 µs to resolve, 0.81 µs to resume). uffd-wp costs 4.74 µs with a handler thread and 1.85 µs in SIGBUS mode
  ([Peter Xu, uffd-wp latency][xzpeter]).
- [F] On arm64 Android, Wine's write watches cost "1–3 µs" per SIGSEGV untraced, about 45 µs under ptrace (proot),
  and 1.45 µs with uffd-wp. RDR2 takes about 2400 such faults a second while riding ([DroidDeck PR 1333][droiddeck]).
  This is a real-game measurement of fault rates on ARM.

## 4. Windows

- [F] Since Windows 10 build 10525 the Memory Manager compresses infrequently used pages into a compression store
  held in RAM, "typically by 40 percent", before writing anything to the page file ([Petri][petri],
  [HotHardware][hh]).
- [F] The store uses Xpress (plain LZ77), lives in the MemCompression process and keeps compressed pages in 16-byte
  chunks indexed by a B+tree. A fault on a page in the store is a "soft" fault: it decompresses the page back into
  the working set ([Black Hat 2019 white paper][bh19], [Fortuna][fortuna]).
- [F] Windows games and runtimes also get `MEM_RESET`, `OfferVirtualMemory` and `DiscardVirtualMemory`
  ([Raymond Chen][oldnew]), and `GetWriteWatch` / `MEM_WRITE_WATCH`. Wine implements write watches with page
  protection and a fault per first write ([Wine virtual.c][winevirt], [DroidDeck PR 1333][droiddeck]).

Assumption Windows games make [H]: committing memory is cheap until it is touched, cold data goes to the page file at
no cost to the game, and free memory is whatever `GlobalMemoryStatusEx` says. On Horizon none of this holds unless we
build it.

## 5. Consoles, emulators, Windows-on-phone stacks

### 5.1 Console title memory models

- [F] PS4: games get 4.5 GB of "direct" memory plus 1 GB of "flexible" memory managed by the OS with FreeBSD
  virtual-memory features. Reportedly 512 MB of it is physical and 512 MB is paged "like a Windows swap file"
  ([Shacknews/Eurogamer report][ps4flex], [VideoGamer: Sony's statement][ps4flex2]).
- [F] PS4 Pro added 1 GB of DDR3 so the OS could move background apps such as Netflix out of GDDR5. That gave games
  another 512 MB, for 5.5 GB in total ([PS4 technical specifications][ps4spec]). The console paged the OS out of the
  game's memory, not the game.
- [F] Xbox Series: Sampler Feedback Streaming loads only the texture tiles the GPU samples. Microsoft calls it "an
  effective 2.5x multiplier" on memory and I/O, because games sample less than a third of loaded texture data
  ([Xbox Velocity Architecture][xvel]). This is paging done by the application with GPU feedback, not by the OS.
- Lesson [H]: consoles give the title a fixed budget and no transparent paging, or very little. Console games are
  written for that. PC games are not, so a PC game on a console-like budget needs some of the PC OS behaviour back.

### 5.2 Emulators

- [F] Emulators map guest RAM directly into host memory ("fastmem"). Dolphin uses a SIGSEGV handler to backpatch
  accesses to unmapped MMIO ([Dolphin trampolines][dolphin]). Ryujinx's "host" memory-manager modes map guest memory
  into host memory ([Ryujinx blog][ryu]). RPCS3 maps the 4 GB PS3 space at `g_base_addr` and keeps per-4 KB-page
  metadata ([RPCS3 deepwiki][rpcs3]).
- [F] None of them pages guest memory itself. They rely on the host OS and require RAM instead: yuzu on Android warns
  below 8 GB ([yuzu progress report June 2023][yuzu]), and RPCS3 needs 8 GB minimum ([RPCS3][rpcs3]).
- [F] Winlator (Wine with Box64 on Android) does nothing special about memory. When Android runs out of RAM, lmkd
  kills the container, and guides recommend 6 GB or more and closing other apps ([XDA Winlator guide][xda-winl]).
  Android itself gives these apps zram system-wide ([Android low-RAM guide][alowram]).
- Takeaway [H]: no emulator or translator we can copy does user-space paging. Carafe would be first on this path,
  except Autorun's own POC (section 7).

## 6. Compression ratios and speeds

### 6.1 Ratios

| Source | Data | Result |
|---|---|---|
| Apple [F] [mav] | macOS LRU pages, WKdm | more than 50 % smaller |
| Windows [F] [petri] | cold pages, Xpress | about 40 % smaller |
| Android [F] [alowram] | app pages in zram | compressed to 30–50 % of the original |
| Google fleet [F] [gfm] | cold server pages, a "lightweight" codec in zswap | median 3×, range 2–6× |
| zstd README / lzbench [F] [ikarus] | Silesia corpus | lz4 2.10×, lzo1x 2.11×, zstd -1 2.88× |
| Oodle Texture [F] [oodle] | BC7 textures with Kraken (a much stronger codec than LZ4) | 1.39:1; BCn mix 1.76:1 |

Inferences for game memory [H]:

- Game heaps, Wine's heaps and x86 code should compress 2–3×. They resemble app memory.
- Zero and same-filled pages are often a large share and cost nothing.
- Block-compressed textures (DXT/BCn), compressed audio and already compressed assets will barely compress with LZ4
  (≤ 1.1×). They must be rejected fast, as XNU's 2048-byte threshold and zram's "huge" pages do.
- Uncompressed RGBA textures and decoded PCM should land in between.
- No published per-page data exists for PC games under Wine. Measuring this is the first experiment (section 9).

### 6.2 Speeds

- [F] x86 desktop reference (lzbench / zstd README): lz4 compresses at 740–780 MB/s and decompresses at
  3.7–5.0 GB/s; zstd -1 at 470–530 and 1380–1700 MB/s; lzo1x at about 650 and 830–845 MB/s ([ikarus][ikarus],
  [lzbench][lzbench]). Arm Neoverse servers: lz4 at 638–766 MB/s and 3.9–5.3 GB/s ([Arm learning path][armlz]).
- [F] Older ARM data: Cortex-A9 (ARMv7), LZ4 r88: 46 MB/s compression, 154 MB/s decompression
  ([LKML 2013][lkml-lz4]). lzo-rle gives "50–300 MB/s" more on zero-heavy pages on arm64 and is the zram default
  since Linux 5.1 ([LWN][lwn-rle]).
- I found no published LZ4 or zstd numbers for the Cortex-A57 or Tegra X1. OpenBenchmarking has Tegra X1 runs, but
  the pages refused access.
- [H] Estimate for an A57 at about 1 GHz. Clock is about 4.5× below a desktop i7, and the A57's IPC on this branchy
  byte-level code is about 0.5–0.6× that of a modern x86, so it is about 8–10× slower overall. That gives:
  - LZ4: about 80–100 MB/s compression (40–50 µs per 4 KB page) and 400–600 MB/s decompression (7–10 µs);
  - zstd -1: about 50–60 MB/s and 150–200 MB/s (70–80 µs and 20–27 µs per page);
  - WKdm: likely as fast as LZ4 or faster on pointer-heavy pages (it works on words), with a lower ratio on byte data.

  Zero-page detection runs at memory bandwidth and is far cheaper than any codec. All of these need one micro-test
  on the console (section 9).

## 7. User-space paging on Horizon

### 7.1 What exists in Autorun today

Read in `carafe/external/autorun`; not run.

- [F] Demand commit: anonymous reservations of 256 MB or more are backed lazily in 2 MB chunks, on the first fault
  (`dlls/ntdll/unix/horizon.c`, `HORIZON_LAZY_MAPPING_MIN`, `horizon_commit_lazy_fault`). Smaller commits are backed
  in full immediately.
- [F] An experimental SD paging proof of concept, CMake option `WINE_NX_SWAP_POC`, OFF by default
  (`horizon-wine/CMakeLists.txt:860`). It needs the per-thread FEX exception entry and the 39-bit address space, and
  the launcher's `swap-mb` turns it on (`horizon-wine/source/runtime.c` around line 3971). How it works:
  - storage: up to eight preallocated 1 GB files in 4 KB units (`swap_store.h`, `swap_file.h`);
  - reclaim: runs when a native allocation fails, or when free heap drops below the request plus 128 MB
    (`swap_pressure_locked`);
  - victims: a cursor walks the mapping tree in address order and skips executable, pinned, excluded and shared
    mappings (`swap_reclaim_locked`);
  - sequence: unmap, then write, then free;
  - swap-in: allocate, read, map, then discard the slot (`horizon_swap_in`);
  - pins for buffers handed to the kernel (`horizon_swap_pin_begin`);
  - a reclaim hook in Vulkan memory allocation (`dlls/win32u/vulkan.c:1455`).
- [H] Compared with the OS designs above, the POC lacks four things:
  1. Access information: the cursor is FIFO by address, not LRU.
  2. A compressed tier: every eviction goes to SD.
  3. Keeping the slot after swap-in. Linux keeps a page's swap slot while the page stays clean, so evicting it again
     costs no write. The POC discards the slot, so every re-eviction rewrites the page.
  4. A write budget.

### 7.2 The fault path

- [F] Horizon facts already established in Carafe's research: guest pages are aliases of heap pages, mapped with
  `svcMapProcessCodeMemory`; changing permissions on part of a block splits it; unmapping needs TLB maintenance
  ([memory-pages.md](../../orchestrator/findings/memory-pages.md)).
- [F] The A57 (ARMv8.0) has no hardware Access/Dirty flag management, which arrived with ARMv8.1 FEAT_HAFDBS
  ([Linux ARM64_HW_AFDBM][afdbm]). User space cannot read Horizon's page tables either. Victim choice must therefore
  use protection-based sampling. This is the 1981 BSD-on-VAX technique: Babaoglu and Joy emulated reference bits
  with protection bits on a machine that had none ([SOSP 1981][bj81], [OSTEP ch. 23][ostep]).
- [H] Cost per fault: an exception entry, the handler, then a map system call (page table update and TLB
  maintenance), then `svcReturnFromException`. Linux arm64 costs 1–3 µs for the signal part alone [F]
  ([droiddeck]). On a 1 GHz A57 with Horizon's path, I expect 5–15 µs before decompression, plus 7–10 µs for an LZ4
  page. The orchestrator's P4 check already plans to measure this "write-fault cost"
  ([frame-pipelining.md](../../orchestrator/findings/frame-pipelining.md)).
- [H] Budget: at 47 fps a frame is about 21 ms. 200 compressed-RAM faults per frame at about 20 µs is 4 ms, a 20 %
  hit. 20 faults is 0.4 ms. An SD fault costs 0.7–2 ms at A1-class random IOPS, so 10 per frame is a visible hitch.
  The main thread must almost never fault on SD, and only rarely on the compressed tier.

### 7.3 Choosing victims without access bits

- [H] DAMON-style regions: split guest memory into regions and, once per window, protect one random 4 KB page per
  region with no access. A fault means "region in use": unprotect it and count it. Regions with no hits over N
  windows are cold. Merge and split regions as DAMON does, so the overhead is bounded by the region count ([damon]).
- [H] Cost: R regions per window means at most R sampling faults. 1000 faults a second at about 10 µs is 10 ms/s,
  1 % of one core.
- [H] Drive the windows with events we already have, such as presented frames (every N frames) and loading screens,
  not a timer. This fits the project's "events, not rates" rule.
- [H] Pitfall: sampling one 4 KB page inside a 2 MB block splits that block for good, which costs TLB reach
  ([memory-pages.md](../../orchestrator/findings/memory-pages.md)). Sample only in regions that are 4 KB-mapped
  already (most guest data, per that document), or accept the split only on regions that are candidates for eviction
  anyway.
- [H] Pages written since the last pass need write tracking too: write-protect after swap-in, and the first write
  marks the page dirty and drops the slot. Wine's write-watch machinery (`VPROT_WRITEWATCH`) already does
  write-protect plus fault ([winevirt]).

### 7.4 Pages that must never be paged

These are [H], derived from how the hardware and kernel work, and the POC's pins and exclusions point the same way:

- Memory mapped into the GPU (nvmap or Vulkan device memory, persistently mapped buffers). The GPU cannot take a
  fault that we service.
- Buffers passed to system calls and IPC (fs reads into guest buffers, audio). The kernel fails on an unmapped page
  instead of raising our exception. They must be pinned for the duration of the call.
- FEX's code cache, the pager's own metadata, stacks of the thread handling the fault, and the exception path itself.
- Pages FEX write-protects for self-modifying code detection. The pager and FEX must agree on who owns a fault. The
  POC requires the per-thread FEX exception entry for this reason.

### 7.5 Writing to SD

- [F] Random 4 KB writes amplify 2–5× ([atp]), and A1 cards guarantee only 500 random-write IOPS ([sda]).
- [H] Design rules:
  - Write compressed data in large sequential segments, a log-structured ring of 256 KB–1 MB, never scattered 4 KB
    pages. This keeps write amplification close to 1 and doubles effective throughput.
  - Keep the slot after swap-in until the first write (the Linux swap cache), so clean evictions cost no write.
  - Cap writes per session or per day, as iOS (1 GB/day for the freezer [F], [xnu-freeze]) and zram
    (`writeback_limit` [F]) do.
  - Read whole clusters of 16–64 KB on swap-in to amortise the latency of about 1 ms.
- [H] Wear arithmetic:
  - a 128 GB TLC card at about 1000 effective P/E cycles and WAF 1.5: about 85 TB;
  - a 32 GB QLC card at 500 P/E cycles and WAF 3: about 5 TB;
  - at 2 GB of writes per hour of play, the QLC card lasts about 2500 hours, if all wear went to swap.

  This is acceptable with a budget and dangerous without one.
- [H] Never put the swap file on the internal eMMC (the NAND install target). It is soldered, so wearing it out
  damages the console, not a removable card.

## 8. Other techniques, briefly

- [H] The "app contract" on Windows. Games size their caches from `GlobalMemoryStatusEx`, from DXGI or D3D9 video
  memory (`GetAvailableTextureMem`), and from `CreateMemoryResourceNotification`. Reporting a truthful, smaller budget
  is the console analogue of iOS footprint limits: zero CPU, large effect on engines that scale texture pools from
  it. Autorun has `horizon_swap_get_memory_info`. Other findings files should own the per-API details.
- [H] Purgeable caches in our own layers: FEX's code cache (re-translation costs CPU, not correctness), DXVK and Mesa
  pipeline and staging caches, and Wine heap free lists. Shrink these on a pressure event. This is the
  `didReceiveMemoryWarning` contract applied inside the runtime.
- [H] KSM-style merging: low value beyond zero pages. Hashing every page costs CPU on a 4-core A57, and game data
  rarely duplicates except for zeros ([ksm] notes that profit can be negative).
- Not applicable:
  - jetsam and lmkd: we have a single process;
  - App Nap: it saves energy, not memory;
  - THP as a memory saver: it is a TLB technique.

## 9. Measurements needed before building

| Name | What | Decides |
|---|---|---|
| `codec-a57` | LZ4, lzo-rle, zstd -1 and WKdm over a buffer of real guest pages on the console; MB/s and µs per page, compression and decompression | codec choice and the background-thread budget |
| `page-census` | For one running game: share of zero pages, same-filled pages, and the LZ4 ratio distribution per 4 KB page, split into heap, image, GPU-mapped and other | the gain of items 1, 3 and 4 below; no published data exists |
| `fault-a57` | The P4 test: protect, fault, map, resume, in µs (already in the orchestrator's list) | fault budgets per frame, sampling rate |
| `sd-random` | Random 4 KB and 64 KB reads and writes through Horizon's fs on the user's cards | whether an SD tier is usable at all, and the cluster size |

## Fit for Carafe (ranked)

Expected gains are [H] until `page-census` is measured. "Pool" means about 3.2 GB.

| # | Technique | Expected gain | CPU / IO cost | Complexity | Risks on 4× A57 + SD |
|---|---|---|---|---|---|
| 1 | **Zero and untouched pages**: demand commit for all commits, not only those of 256 MB or more; scan and release all-zero pages; map them again on fault (zram same-filled, KSM `use_zero_pages`) | Potentially the largest, from 0 to hundreds of MB for games that over-commit, as PC games can afford to | Scan at memory bandwidth on a background core; one fault per first touch (about 10 µs) | Low to medium: the lazy-fault path exists | Fault bursts at first touch (use 64 KB–2 MB chunks); GPU and IPC buffers must be excluded |
| 2 | **Honest memory contract**: what `GlobalMemoryStatusEx`, DXGI and D3D9 report, low-memory notifications, plus purgeable caches in FEX, DXVK and Wine shrunk on a pressure event | Tens to hundreds of MB where engines size pools from these values; tens of MB from our caches | Nil; re-translation or re-creation on demand | Low | Some games refuse to start below a stated RAM minimum; reported values must stay consistent |
| 3 | **Drop clean pages, re-read from the file**: unmodified image pages (game EXE/DLLs, Wine's PE DLLs), read-only file mappings | Tens of MB, more for big executables and mapped archives | Zero writes, no wear; about 1 ms per cluster re-read | Medium: needs "clean" tracking (write-protect) and per-page relocation state | Relocated pages are not clean (fixed-base topic); FEX reads guest code when it translates new blocks; SMC protection interplay |
| 4 | **Compressed-RAM tier** (Windows store / zram / XNU compressor): LZ4, or a hybrid with fast rejection over 3 KB; cold pages from protection sampling (DAMON-style, Babaoglu–Joy) | About 2–3× on cold heap data; if a third of the pool is cold and compressible, about 0.5–0.7 GB | About 40–50 µs per page to compress on a background core (about 100 MB/s); 15–25 µs per fault to restore; sampling about 1 % of a core | Medium to high: on top of the POC's pins, exclusions and unmap/map, add sampling, a compressed allocator and per-frame fault caps | Main-thread faults add frame-time spikes; splitting 2 MB blocks costs TLB reach; incompressible textures waste CPU without a fast reject |
| 5 | **SD tier behind the compressed tier** (zswap writeback / XNU swapfiles): log-structured compressed segments, slot kept until the first write, session write budget | Capacity in GB for data that is cold for good (load-time data, unused levels) | About 1 ms per random read; writes cost wear | High: the POC's file store exists, and the rest is new | Hitches if a hot page lands there; wear on cheap QLC cards; never on internal eMMC; behaviour across sleep and an ejected card |
| 6 | Write tracking for re-eviction (swap-cache semantics, the uffd-wp analogue) | Removes most rewrites in tiers 4 and 5 | One write fault per page after swap-in | Low once 4 or 5 exist | Same fault-ownership issues with FEX |
| 7 | KSM-style merging of non-zero pages | Small | Hashing on a CPU-bound device | Medium | Negative profit ([ksm]) |
| — | Jetsam or lmkd, App Nap, THP as a saver | None for a single game process | — | — | — |

Order of work this suggests [H]:

1. Run `page-census` and `codec-a57`: data first.
2. Build item 2, the cheapest.
3. Build item 1.
4. Build item 3 for images.
5. Build item 4 on the POC's foundations.
6. Build item 5 only for games that still run out of memory, behind an Experimental option that is off by default.

[mav]: https://www.apple.com/media/us/osx/2013/docs/OSX_Mavericks_Core_Technology_Overview.pdf
[ai-mav]: https://appleinsider.com/articles/13/06/13/compressed-memory-in-os-x-109-mavericks-aims-to-free-ram-extend-battery-life
[wk99]: https://www.usenix.org/legacy/event/usenix99/full_papers/wilson/wilson.pdf
[xnu-alg]: https://github.com/apple/darwin-xnu/blob/main/osfmk/vm/vm_compressor_algorithms.c
[xnu-arm64]: https://github.com/apple/darwin-xnu/tree/main/osfmk/arm64
[xnu-bs]: http://newosxbook.com/code/xnu-3247.1.106/osfmk/vm/vm_compressor_backing_store.h.auto.html
[macint]: https://www.macinternals.app/en/blog/memory-pressure-on-macos
[wwdc416]: https://asciiwwdc.com/2018/sessions/416
[purg]: https://developer.apple.com/library/archive/documentation/Performance/Conceptual/ManagingMemory/Articles/CachingandPurgeableMemory.html
[offer]: https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-offervirtualmemory
[reclaim]: https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-reclaimvirtualmemory
[oldnew]: https://devblogs.microsoft.com/oldnewthing/20170113-00?p=95185
[xnu-notify]: https://github.com/apple-oss-distributions/xnu/blob/main/doc/vm/memorystatus_notify.md
[levin]: http://newosxbook.com/articles/MemoryPressure.html
[ios-ent]: https://zenn.dev/mtfum/articles/ios_memory_entitlements?locale=en
[xnu-freeze]: https://github.com/apple/darwin-xnu/blob/main/bsd/sys/kern_memorystatus_freeze.h
[appnap]: https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/AppNap.html
[zram]: https://docs.kernel.org/admin-guide/blockdev/zram.html
[zswap]: https://docs.kernel.org/admin-guide/mm/zswap.html
[gfm]: https://andres.lagarcavilla.com/publications/LagarCavillaASPLOS2019.pdf
[gfm-sum]: https://blog.acolyer.org/2019/05/22/sw-far-memory/
[alowram]: https://android.googlesource.com/platform/docs/source.android.com/+/bb6e1eb86b31dd2d824a3639d7994dac8dffa9e4/en/devices/tech/perf/low-ram.html
[jhacks]: https://jetsonhacks.com/2019/11/28/jetson-nano-even-more-swap/
[swdeck]: https://github.com/SildurFX/Switchdeck
[swroot]: https://wiki.switchroot.org/wiki/linux/linux-changelog
[zram-wb]: https://lkml.iu.edu/hypermail/linux/kernel/1811.3/02145.html
[sda]: https://www.sdcard.org/developers/sd-standard-overview/application-performance-class/
[atp]: https://www.atpinc.com/blog/choosing-memory-cards-for-dashcam-usage
[htsemi]: https://htsemi.net/microsd-card-life-estimation-guide-tbw-p-e-cycles-for-security-cameras-and-dash-cams-ht-semi/
[mglru]: https://docs.kernel.org/admin-guide/mm/multigen_lru.html
[mglru-ph]: https://www.phoronix.com/news/Linux-Multigen-LRU
[damon]: https://docs.kernel.org/mm/damon/design.html
[ksm]: https://docs.kernel.org/admin-guide/mm/ksm.html
[lmkd]: https://source.android.com/docs/core/perf/lmkd
[uffd]: https://docs.kernel.org/admin-guide/mm/userfaultfd.html
[xzpeter]: https://xzpeter.org/userfaultfd-wp-latency-measurements/
[droiddeck]: https://github.com/Droid-Deck/DroidDeck/pull/1333
[petri]: https://petri.com/memory-compression-in-windows-10-threshold-2/
[hh]: https://hothardware.com/news/microsoft-returns-to-testing-mode-releases-windows-10-build-10525-to-insiders
[bh19]: https://i.blackhat.com/USA-19/Thursday/us-19-Sardar-Paging-All-Windows-Geeks-Finding-Evil-In-Windows-10-Compressed-Memory-wp.pdf
[fortuna]: https://andreafortuna.org/2019/08/01/forensic-analysis-of-windows-10-compressed-memory-using-volatility/
[winevirt]: https://gitlab.winehq.org/wine/wine/-/blob/wine-7.22/dlls/ntdll/unix/virtual.c
[ps4flex]: https://www.shacknews.com/article/80367/report-ps4-developers-can-only-access-up-to-55gb-of
[ps4flex2]: https://www.videogamer.com/news/ps4s-flexible-memory-is-easy-for-every-developer-to-use-says-sony/
[ps4spec]: https://en.wikipedia.org/wiki/PlayStation_4_technical_specifications
[xvel]: https://news.xbox.com/en-us/2020/07/14/a-closer-look-at-xbox-velocity-architecture/
[dolphin]: https://www.tumblr.com/accidentallyquadratic/142829260822/dolphin-emulator-trampoline-generation
[ryu]: https://blog.ryujinx.org/ryujinx-power/
[rpcs3]: https://deepwiki.com/RPCS3/rpcs3/2.5-memory-and-thread-synchronization
[yuzu]: https://yuzu-emu.org/entry/yuzu-progress-report-jun-2023/
[xda-winl]: https://www.xda-developers.com/winlator-guide/
[ikarus]: https://ikarus.sg/using-zram-to-get-more-out-of-your-raspberry-pi/
[lzbench]: https://github.com/inikep/lzbench
[armlz]: https://learn.arm.com/learning-paths/servers-and-cloud-computing/snappy/compression/
[oodle]: http://cbloomrants.blogspot.com/2020/06/oodle-texture-slashes-game-sizes.html
[lkml-lz4]: https://lkml.iu.edu/1302.3/01029.html
[lwn-rle]: https://lwn.net/Articles/778510/
[afdbm]: https://cateee.net/lkddb/web-lkddb/ARM64_HW_AFDBM.html
[bj81]: https://www.cs.unibo.it/babaoglu/papers/pdf/sosp81.pdf
[ostep]: https://pages.cs.wisc.edu/~remzi/OSTEP/vm-complete.pdf
