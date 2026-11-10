# What Windows does with memory, and what games rely on

Research snapshot: 2026-11-05. For subsequent runtime changes and validation, see the
[report](../REPORT.md#updates-after-the-study).

Research for the memory study, 2026-11-05. Read-only: Microsoft Learn, Windows Internals knowledge,
the Autorun Wine tree (`external/autorun`, the reference implementation Carafe runs) and the DXVK
3.1.1 source Carafe builds (`runtime/build/dxvk-src`, revision `b1a1c99`, pinned in
`runtime/dxvk/build-dxvk.sh`).

Each section separates **Fact** (with a source), **Carafe today** (what the code does, with file and
function) and **Hypothesis** (not proven, labelled).

## 1. The one-sentence difference

On Windows a page costs physical memory only while it is resident, and almost every resident page
can be made non-resident without the program noticing: clean file pages are dropped and re-read,
dirty private pages go to the pagefile or the compression store, and committed pages that were never
touched were never resident at all. On Carafe today nearly every committed or mapped page is backed
by heap memory from the moment it is committed or mapped, stays backed until it is freed, and cannot
be dropped. Games were written against the first model.

## 2. Reserve, commit, commit charge, demand-zero

**Fact.**
- `MEM_RESERVE` takes address space only, "without allocating any actual physical storage in memory
  or in the paging file". `MEM_COMMIT` "allocates memory charges (from the overall size of memory and
  the paging files on disk)… Actual physical pages are not allocated unless/until the virtual
  addresses are actually accessed", and the first access reads zeros.
  Source: [VirtualAlloc, Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc).
- The commit limit is RAM plus the pagefiles; the commit charge of all processes cannot exceed it,
  and when it is reached committing allocations fail. The pagefile grows by default, so the limit
  rises when the charge nears it. Private and pagefile-backed memory are charged; memory that
  represents a file on disk is not, since Windows can always re-read it.
  Source: Mark Russinovich, [Pushing the Limits of Windows: Virtual Memory](https://learn.microsoft.com/en-us/archive/blogs/markrussinovich/pushing-the-limits-of-windows-virtual-memory).
- A first touch of a committed page is a *demand-zero fault*, a soft fault resolved without any
  backing store. Source: [Working Set, Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/memory/working-set).
- So on Windows committed-but-untouched memory costs commit charge (a number) and page-table space,
  never RAM. With a system-managed pagefile the commit limit is typically far above RAM, so a game
  that commits 2 GB "just in case" and touches 600 MB uses 600 MB of RAM.

**Carafe today** (`dlls/ntdll/unix/virtual.c`, `dlls/ntdll/unix/horizon.c`):
- Wine's `map_view` maps a committed view with read/write protection straight away, and a commit of a
  reserved range is an `mprotect`. Both land in `horizon_mmap_*` / `protect_range_locked`.
- `create_backing_locked` takes the pages from the pool (`horizon_pages_alloc_any`) and `memset`s
  them to zero. Commit therefore costs heap memory immediately, and the memset touches every page.
- A commit of part of a reservation maps the whole 64 KB chunk around it (`HORIZON_COMMIT_CHUNK`,
  needed because Horizon has 20000 memory blocks for all applications). Pages the program did not
  commit stay inaccessible but are backed.
- Exception: an anonymous mapping of 256 MB or more (`HORIZON_LAZY_MAPPING_MIN`) is a "lazy"
  reservation; `horizon_commit_lazy_fault` backs it in 2 MB chunks (`HORIZON_POOL_ARENA`) on first
  fault. This is real demand-zero, but only for very large single mappings.
- libnx takes the whole application heap from Horizon at start-up, so "physical memory" for the
  game means "free space in this process's heap" (comment in `horizon_get_memory_info`).

**Hypothesis.** Many games commit far more than they touch: custom heaps committed up front
(Gamebryo/Creation-engine heaps, engine "pools"), thread stacks (Windows commits only the initial
part and grows it through a guard page), .NET/Mono GC segments, DXVK and driver staging pools. No
measurement exists yet. The lazy path already produces the number for large maps (`lazy_va_mb` in the
swap report, only in the `WINE_NX_SWAP_POC` build, which is off: `CMakeCache.txt`
`WINE_NX_SWAP_POC:BOOL=OFF`). A per-game "committed vs touched" counter is the first thing to add.

## 3. Decommit, MEM_RESET and the "give it back" hints

**Fact.**
- `VirtualFree(MEM_DECOMMIT)` releases the page's storage immediately, page by page
  ([VirtualAlloc remarks](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualalloc)).
- `MEM_RESET`: data "is no longer of interest. The pages should not be read from or written to the
  paging file. However, the memory block will be used again later, so it should not be decommitted."
  Contents afterwards are undefined (not guaranteed zero). `MEM_RESET_UNDO` (Windows 8+) tries to get
  the data back and fails if any of it was replaced with zeros. Same source.
- `DiscardVirtualMemory` (Windows 8.1 Update): discards contents, keeps the region committed, "may
  give physical RAM back to the system"; on next access "the backing RAM is restored, and the contents
  of the memory is undefined". Requires `PAGE_READWRITE`.
  Source: [DiscardVirtualMemory](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-discardvirtualmemory).
- `OfferVirtualMemory` / `ReclaimVirtualMemory` (Windows 8.1 Update): offered pages "will be marked as
  inaccessible, removed from the process working set, and will not be written to the paging file";
  four priorities (`VMOfferPriorityVeryLow`…`Normal`); reclaim reports whether the data survived.
  Source: [OfferVirtualMemory](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-offervirtualmemory).
- Who uses them (well known, not re-verified here): allocators such as jemalloc, mimalloc and
  tcmalloc on Windows decommit or `MEM_RESET` free spans; the CoreCLR GC resets free regions
  (`VirtualReset`); Chromium/CEF (used by launchers and some game UIs) uses `DiscardVirtualMemory`
  and `OfferVirtualMemory`.

**Carafe today.**
- `MEM_RESET` reaches `madvise(MADV_DONTNEED)` (`virtual.c`, `allocate_virtual_memory`), and
  `horizon_madvise` ignores every advice (`horizon.c`: `"madvise(%p, %zu, %#x) ignored."`). So
  `MEM_RESET` and `DiscardVirtualMemory` (implemented in `kernelbase/memory.c` as `MEM_RESET`) free
  nothing. `MEM_RESET_UNDO` is accepted and trivially "succeeds", which is correct.
- `OfferVirtualMemory` and `ReclaimVirtualMemory` are not exported at all (commented stubs in
  `kernelbase.spec`). A game linking them statically would fail to load; one using `GetProcAddress`
  falls back.
- `PrefetchVirtualMemory` is `madvise(MADV_WILLNEED)`, also ignored (harmless).
- Decommit (`decommit_pages`) maps the range `PROT_NONE`. In `horizon_mmap_fixed` this unmaps the
  range; `split_backing_mapping` keeps the backing alive while any other piece of it is mapped
  (`release_backing` only frees at zero references). An anonymous mapping is built from backings of up
  to 16 MB (`map_anonymous_backings`, `HORIZON_POOL_ARENA * 8`). **Decommitting part of a backing
  frees nothing** until the last piece of that backing goes. On Windows the same call frees exactly the
  decommitted pages.

## 4. File mappings, image sections and the page cache

**Fact** (Windows Internals 7th ed., part 1, ch. 5; Working Set page above):
- File data read with `ReadFile` lives in the system file cache, not in the process; it is not charged
  to the process and is evicted under pressure.
- A mapped view of a file uses the file as its backing store. Clean pages are discardable: when
  trimmed they go to the standby list and can be repurposed at once, and a later access re-reads them
  from the file (hard fault) or takes them back from standby (soft fault). Several views of one file,
  in one or several processes, share the same physical pages.
- Image sections (`SEC_IMAGE`) are mapped the same way. Code and read-only data are shared and
  discardable. Writable sections are copy-on-write: a page becomes private (and pagefile-backed) only
  when written. `.bss` (virtual size beyond raw data) is demand-zero.
- The modified list holds dirty pages waiting to be written to their backing store; standby holds
  clean ones. Memory priority decides which standby pages are repurposed first.

**What engines rely on (mixed).**
- *Fact, general:* .NET and Mono map managed assemblies as file or image views; DXVK's own 32-bit D3D9
  path keeps texture data in pagefile-backed sections (section 7).
- *Hypothesis:* Unity's player reads `resources.assets`, `.resS` and AssetBundles through its own
  async file reader rather than mapping them; `.resS` streaming reads mip data on demand. If so the
  Windows-side benefit Unity gets is the file cache (speed), not mapping. Unreal 3 reads its packages
  (`.upk`, `.xxx`) with file I/O; UE4 can map pak files on some platforms. Not verified in source; the
  view census below answers it per game.

**Carafe today.**
- A view of a real file goes `map_file_into_view` → `mmap(fd)` → `horizon_mmap_fixed` →
  `map_backing_at` → `create_backing_locked`, which allocates the whole view, zeroes it and reads the
  file into it (`read_fd_at`). Writable shared views keep the fd and write back on unmap. So **every
  file view costs its full size in heap, read eagerly, for its whole lifetime**, and two views of one
  file are two copies.
- Images: `map_image_view` creates the view committed and read/write, then each section is mapped or
  `pread` into it (`map_image_into_view`, `map_pe_header`). Either way every image is fully resident
  and private, including `.bss` and code that never runs.
  *Hypothesis to check:* if the `mmap` path (not `pread`) is taken for sections, the anonymous backing
  made for the whole view stays referenced under the file-backed pieces (same partial-unmap rule as
  section 3), so an image could cost close to twice its size. Wine passes `needs_close` as `removable`,
  which usually selects `pread`; a `[HMAP]` trace of one image load settles it.
- Pagefile-backed sections (`CreateFileMapping(INVALID_HANDLE_VALUE)`) are `horizon_memfile`s;
  `horizon_memfile_alloc` allocates and zeroes the full size even for `SEC_RESERVE` (it only keeps a
  "committed" byte per page for bookkeeping).
- `wine_nx_log_views` (in `virtual.c`) already prints image / file / anonymous totals when the address
  space runs out; the same census on demand would answer "how much of this game's memory is file
  copies".

## 5. Working set, trimming, compression

**Fact.**
- The working set is the resident part of the process. Pages leave it through
  `SetProcessWorkingSetSize(Ex)` / `EmptyWorkingSet`, `VirtualUnlock` on unlocked memory,
  `UnmapViewOfFile`, or memory-manager trimming; a trimmed page stays cached as a transition page and
  dirty ones are written to their backing store before reuse.
  Source: [Working Set](https://learn.microsoft.com/en-us/windows/win32/memory/working-set).
- Memory priority (`SetProcessInformation(ProcessMemoryPriority)`, `MEMORY_PRIORITY_VERY_LOW` 1 …
  `MEMORY_PRIORITY_NORMAL` 5, default 5) is "a hint to the memory manager when it trims pages from the
  working set"; lower-priority pages go first.
  Source: [MEMORY_PRIORITY_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-memory_priority_information).
- Windows 10 (1511) added the compression store: modified private and pagefile-backed pages are
  compressed into a store owned by the Memory Compression process instead of being written to disk
  at once; a fault on such a page decompresses it (Windows Internals 7th ed., "Memory compression").
  This is why a Windows PC with little RAM and a small pagefile still keeps games alive.

**Carafe today.**
- `SetProcessWorkingSetSizeEx` returns `TRUE` and does nothing; `EmptyWorkingSet` calls it
  (`kernelbase/process.c`, `kernelbase/debug.c`). Correct behaviour, no effect.
- `SetProcessInformation(ProcessMemoryPriority)` goes to `NtSetInformationProcess(ProcessPagePriority)`,
  which falls to the `default:` case and returns `STATUS_NOT_IMPLEMENTED`, so the call returns FALSE.
  Windows returns TRUE. Probably harmless (callers rarely check), but it is a visible difference.
- There is no trimming, no standby list and no compression. The only paging code is the SD-backed
  swap experiment (`WINE_NX_SWAP_POC`, off).

## 6. Signals and sizing: what games ask before they allocate

### GlobalMemoryStatusEx

**Fact.** Engines and launchers read `ullTotalPhys`, `ullAvailPhys`, `ullTotalPageFile` (the commit
limit) and `ullAvailPageFile` to pick quality presets, cache sizes and pool sizes, and some refuse to
start below a threshold (Carafe already met Fallout New Vegas wanting 512 MB, and Wine's comment
"Titan Quest refuses to run if TotalPageFile <= TotalPhys").

**Carafe today** (`ntdll/unix/system.c`, `virtual.c`, `horizon_get_memory_info`):
- `TotalPhys` = `InfoType_TotalMemorySize` of the process.
- `AvailPhys` = total − used, where "used" is corrected to "heap not yet handed out + malloc free".
- `TotalPageFile` = total + 4 KB (Wine's fake one-page swap); `AvailPageFile` = the same free figure.
- `GlobalMemoryStatusEx` caches the result for one second (upstream Wine behaviour).
- So a game sees "no pagefile, commit limit = RAM", which is accurate for Carafe and unusual for
  Windows. The free figure falls as memory is used, which is the right signal, but nothing else
  reacts to it.

### CreateMemoryResourceNotification

**Fact.** Low and high memory notifications are system-wide events signalled when available physical
memory is low or high, with a band in between where neither is set; applications are meant to shrink
when low is signalled.
Source: [CreateMemoryResourceNotification](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification).

**Carafe today.** kernelbase opens `\KernelObjects\LowMemoryCondition` / `HighMemoryCondition`. Upstream
wineserver creates them once (low unsignalled, high signalled) and never changes them
(`server/directory.c`). The Horizon in-process server opens named events by lookup
(`horizon_server_handle_open_named_object`), and nothing in `horizon.c` creates these two names, so
**the call most likely fails and returns NULL** (to be confirmed with one trace). Either way no game
on Carafe can ever receive a low-memory signal.

### DXGI video-memory budget

**Fact.** In D3D12 (and D3D11 titles that use DXGI 1.4) the budget comes from
`IDXGIAdapter3::QueryVideoMemoryInfo`; apps are expected to watch
`RegisterVideoMemoryBudgetChangeNotificationEvent` and destroy unused objects when the budget drops.
UMA adapters have one pool. Source: [Residency, Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/direct3d12/residency).

**Carafe today (DXVK 3.1.1, `src/dxgi/dxgi_adapter.cpp`).**
- `QueryVideoMemoryInfo` reports `VK_EXT_memory_budget` heap budget and usage when the driver has the
  extension, otherwise the heap size as budget.
- `RegisterVideoMemoryBudgetChangeNotificationEvent` is implemented: it signals once at registration
  and an event thread re-signals on budget changes (`runEventThread`). It only helps if the Switch
  Vulkan driver's budget moves with real free memory (not verified in this study).
- `DXGI_ADAPTER_DESC`: on a single-heap integrated GPU DXVK reports the heap as
  `DedicatedVideoMemory` **and also** as `SharedSystemMemory` ("over-report… some games may be
  allergic to reporting no shared memory"). A game adding the two, or the two plus `TotalPhys`, sees
  far more memory than exists. `dxgi.maxDeviceMemory` / `dxgi.maxSharedMemory` cap these values; Carafe
  ships no `dxvk.conf`, so they are uncapped.
- D3D9 `GetAvailableTextureMem` = device + system heap sizes, capped by `d3d9.maxAvailableMemory`
  (default 4096 MB), minus 8 MB (`D3D9DeviceEx::DetermineInitialTextureMemory`).

**Hypothesis.** Engines that size a texture pool or streaming budget from dedicated VRAM (common in
D3D9/D3D11-era engines, and Unity reports `SystemInfo.graphicsMemorySize` from it) pick their largest
pool on Carafe, although on the Switch that memory is the same 3.2 GB as everything else.

## 7. Graphics: managed pools, eviction, residency

**Fact.**
- D3D9 `D3DPOOL_MANAGED` resources "exist both in video and system memory"; the runtime copies them
  to video memory when needed and evicts by LRU.
  Source: [Managing Resources (Direct3D 9)](https://learn.microsoft.com/en-us/windows/win32/direct3d9/managing-resources).
  On Windows the system copy is ordinary pageable memory: when unused it is trimmed to the pagefile or
  the compression store.
- D3D11 `SetEvictionPriority`: when video memory runs out the resource "will be removed from video
  memory and placed into system memory, or possibly placed onto the hard drive".
  Source: [ID3D11Resource::SetEvictionPriority](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11resource-setevictionpriority).
- D3D12 `Evict` / `MakeResident`: evicted heaps may be paged out to disk; destroying heaps is the
  reliable way to free residency. Source: Residency page above.

**Carafe today (DXVK 3.1.1).**
- D3D9 managed and system-memory textures keep a CPU copy for their whole life
  (`D3D9_COMMON_TEXTURE_MAP_MODE_BACKED` / `SYSTEMMEM`, `D3D9CommonTexture::DetermineMapMode`). On
  32-bit builds (`DXVK_USE_UNMAPPABLE_MEMORY`, `util/util_unmap.h`) that copy lives in 16 MB
  pagefile-backed sections (`CreateFileMappingA(INVALID_HANDLE_VALUE, … SEC_COMMIT)`) that DXVK maps
  and unmaps by LRU above `d3d9.textureMemory` (100 MB) to save **address space**. On Windows the
  unmapped part is pageable; on Carafe each section is a fully allocated `horizon_memfile`, so the
  unmap saves address space only. The managed copy is a pure duplicate on a UMA device.
- `EvictManagedResources` returns `D3D_OK` and does nothing (`d3d9_device.cpp`).
- D3D11 `SetEvictionPriority` is a logged stub on buffers and textures.
- DXVK's allocator has eviction and defragmentation, but `enforceBudget` is set only for
  non-UMA discrete GPUs (`dxvk_memory.cpp`: `!isUnifiedMemoryArchitecture() && DISCRETE_GPU`).
  Eviction moves resources to system memory, which on the Switch is the same memory. So DXVK will not
  reduce its footprint under pressure on Carafe.
- Carafe's DXVK patch 0012 (`d3d9.managedBufferCopies`) adds a front-end copy of managed buffers that
  are locked more than once; more memory for speed, worth counting in the census.

## 8. How engines react to a failed allocation or a low-memory signal

**Fact.**
- Unity raises `Application.lowMemory` only on iOS, Android and UWP on memory-constrained devices
  (HoloLens, Xbox One), "not on desktop".
  Source: [Application.lowMemory, Unity docs](https://docs.unity.com/en-us/engine/current/script-reference/unityengine/application/lowmemory).
  A Windows Unity player therefore never trims itself on a signal.
- A failed native allocation in Unity is fatal: the log shows "Could not allocate memory: System out
  of memory! Trying to allocate: … MemoryLabel: …" and the player aborts. User reports tie it to
  disabled pagefiles. Sources: [Unity Discussions](https://discussions.unity.com/t/crash-system-out-of-memory-related-to-unity-input/843518),
  [Steam: Derail Valley](https://steamcommunity.com/app/588030/discussions/1/2247805152838923823).
- Carafe evidence (Cat Mail Co, Unity, 2026-11-05 console run, memory note): heap 3030 MB used,
  `create_backing` ENOMEM, a 4 MB `VirtualAlloc` in `Loading.PreloadManager` failed with `c0000022`
  (Windows would report `STATUS_NO_MEMORY` or `STATUS_COMMITMENT_LIMIT`), then an access violation at
  the requested address and a spinning thread.

**Hypothesis.**
- Unreal Engine 3 and 4 treat a failed allocation as fatal (UE4's `FGenericPlatformMemory::OnOutOfMemory`
  prints "Ran out of memory allocating … bytes" and crashes; UE3's `FMallocWindows` follows the same
  pattern). Neither listens to Windows low-memory notifications on PC.
- Consequence for Carafe: on PC, games never see an allocation fail because Windows trims, compresses
  and grows the pagefile first. **Reclaim has to happen inside the failing commit, before it returns**,
  not by asking the game to shrink.

## 9. API status summary

| API | Windows | Wine / Carafe today |
|---|---|---|
| `MEM_COMMIT` | charge only, RAM on first touch | heap at once (64 KB chunk, zeroed); lazy only for ≥256 MB maps |
| `MEM_DECOMMIT` | frees those pages | frees only when a whole ≤16 MB backing is unmapped |
| `MEM_RESET`, `DiscardVirtualMemory` | RAM may be dropped | no-op (`horizon_madvise` ignored) |
| `MEM_RESET_UNDO` | data back unless dropped | succeeds (correct, nothing was dropped) |
| `OfferVirtualMemory` / `ReclaimVirtualMemory` | droppable with priority | not exported |
| `SetProcessWorkingSetSize`, `EmptyWorkingSet` | trims to standby / pagefile | TRUE, no effect |
| `ProcessMemoryPriority` | trimming hint | `STATUS_NOT_IMPLEMENTED`, call returns FALSE |
| `CreateMemoryResourceNotification` | live system events | events not created on Horizon (likely NULL) |
| `GlobalMemoryStatusEx` | RAM + pagefile | process total; pagefile 4 KB; free = heap free |
| file views (`MapViewOfFile`) | shared, clean pages discardable | private eager copy of the whole view |
| image sections | shared, CoW, `.bss` demand-zero | private, fully read at load |
| pagefile sections | charge only, pageable | full size allocated and zeroed, even `SEC_RESERVE` |
| DXGI adapter memory | real VRAM / shared | heap reported as both dedicated and shared |
| `QueryVideoMemoryInfo`, budget event | live | implemented via `VK_EXT_memory_budget` |
| D3D9 managed pool | system copy pageable | system copy pinned (memfile on 32-bit) |
| `EvictManagedResources`, D3D11 `SetEvictionPriority` | evicts | no-op / stub |
| DXVK budget eviction | n/a | only on discrete GPUs, never on UMA |

## What Carafe could emulate

Savings are estimates for a typical 1–3 GB game; each needs a measurement before work starts. Risk
is the chance of breaking games or costing speed.

1. **Measure first: a memory census in the runtime log.** Per category: committed vs touched (fault
   counting on lazily backed ranges), image views, file views, pagefile sections, DXVK managed copies,
   GPU allocations. Extends `wine_nx_log_views` and the `[HMAP]` counters. *Saves nothing itself; tells
   which of the items below pays.* Risk: none (logging only).

2. **Demand-zero commit for all private memory, not only ≥256 MB maps.** Commit keeps the 64 KB
   kernel mapping trick but backs pages on first fault, as `horizon_commit_lazy_fault` already does
   for large maps; drop the eager `memset` (fresh pool pages can be zeroed when handed out). *Saves:*
   committed-but-untouched memory; plausibly 10–30% of private commit in engines with up-front heaps
   (hypothesis). *Risk:* medium. Kernel and IPC copies into a not-yet-backed page fail instead of
   faulting (file reads, GPU uploads), so syscalls and Horizon IPC buffers must pre-fault their ranges;
   fault cost on hot first touches; more 2 MB arenas in use means more kernel memory blocks.

3. **Make `MEM_RESET`, `DiscardVirtualMemory` and `OfferVirtualMemory` real.** Free every whole pool
   arena inside the range and turn it into a lazy range; `MEM_RESET_UNDO` / `ReclaimVirtualMemory`
   report "lost" only for arenas that were freed; export Offer/Reclaim. *Saves:* whatever allocators
   and GCs reset (CoreCLR, jemalloc/mimalloc users, CEF launchers); unknown per game, zero for games
   that never call it. *Risk:* low; the contract already allows undefined contents.

4. **Precise decommit.** Free pool arenas of a backing once no mapping references them, instead of
   only when the whole ≤16 MB backing is gone. *Saves:* memory held by partly decommitted heaps (heap
   managers decommit free pages, Fallout New Vegas commits a page at a time). *Risk:* medium; touches
   the alias-source bookkeeping that already caused faults once (`free_backing` leaks aliased pages on
   purpose).

5. **File views and images as clean, discardable pages (user-space file-backed paging).** Map
   read-only and copy-on-write file views lazily: back a 2 MB chunk on first fault by reading it from
   the file; under pressure drop chunks that were never written and re-read them on the next fault.
   Same for image sections (code, read-only data) and demand-zero `.bss`. This is exactly what Windows
   does and is safe because the file is the backing store. *Saves:* untouched parts of mapped files and
   images (large executables, managed assemblies, any engine that maps its archives), and gives a pool
   of droppable pages to reclaim from. *Risk:* medium-high. SD reads inside a fault stall the thread;
   pages must be pinned around syscalls; FEX's self-modifying-code tracking and translated-code cache
   must learn that a code page can disappear and come back unchanged.

6. **Reclaim inside a failing commit (Windows "grow the pagefile" moment).** When `create_backing`
   returns ENOMEM, drop discardable pages (items 3 and 5), compress cold pages (item 8), then retry
   once before failing; and fail with `STATUS_NO_MEMORY` / `STATUS_COMMITMENT_LIMIT`, not
   `STATUS_ACCESS_DENIED`. *Saves:* turns many fatal out-of-memory aborts into slow moments. *Risk:*
   low for the retry and the status code, the rest depends on items 3, 5 and 8.

7. **Tell games the truth about video memory.** Per-game or default `dxvk.conf`: cap
   `dxgi.maxDeviceMemory` and `dxgi.maxSharedMemory` (e.g. 1 GB / 1 GB) and `d3d9.maxAvailableMemory`
   so engines do not pick their largest texture pools on a device whose "VRAM" is its RAM. *Saves:*
   possibly hundreds of MB of texture pools and streaming caches (hypothesis; depends on the engine).
   *Risk:* low-medium. Some games refuse to start under a VRAM minimum or lower texture quality; needs
   a per-game override and must not go below what the game demands.

8. **A compression store for cold private and section memory (Windows 10 model).** Compress 2 MB
   chunks that have not been touched for a while (the swap experiment's tracking, with RAM as the
   target instead of SD), starting with the cleanest candidates: unmapped DXVK `util_unmap` chunks,
   unmapped pagefile sections, and managed-pool copies. *Saves:* 30–60% of cold data that compresses
   (uncompressed textures, game state); little for already compressed DXT data (hypothesis). *Risk:*
   medium-high: CPU time on the A57 cores, fault latency, the same syscall and FEX caveats as item 5.

9. **Drop the D3D9 managed copy on a UMA device.** Once a managed or system-memory texture is uploaded
   and not locked for a while, free its CPU copy; on the next lock, read it back from the GPU image
   (CPU-visible on the Switch) into a fresh copy. *Saves:* the full size of every managed texture, one
   of the largest duplicates in DX9 games (hypothesis: 100–500 MB). *Risk:* medium. Readback must
   handle tiled and block-compressed formats exactly, locks of evicted textures get slower, and it is
   a DXVK patch (Experimental option per project rules).

10. **Lazy pagefile sections.** Allocate `horizon_memfile` pages when a page is committed or first
    touched instead of the full size at creation, starting with `SEC_RESERVE` (the per-page committed
    map exists already). *Saves:* reserved-but-uncommitted section memory, and untouched parts of
    `SEC_COMMIT` sections. *Risk:* medium, sections are aliased into several views through the kernel.

11. **Live low/high memory signals and small API fixes.** Create `\KernelObjects\LowMemoryCondition`
    and `HighMemoryCondition` in the Horizon server and set them from heap free (Windows-like
    thresholds); return TRUE from `ProcessMemoryPriority` and keep the priority as a hint for item 5/8
    eviction order. *Saves:* little directly (most PC games ignore the events); helps launchers, CEF
    and any engine that listens. *Risk:* low.
