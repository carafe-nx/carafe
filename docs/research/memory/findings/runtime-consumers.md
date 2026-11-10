# Who uses the runtime's memory (Autorun + Carafe patches)

Research snapshot: 2026-11-05. For subsequent runtime changes and validation, see the
[report](../REPORT.md#updates-after-the-study).

Scope: how Autorun (Wine for Horizon) and Carafe's patches use the Switch process's memory, who the consumers of the
~3 GB heap are, and how a failed allocation turns into the status the game sees. Read-only study; nothing was built
or run.

Sources and how to read the citations:

- `autorun/...` is `carafe/external/autorun` at `636913ca` (2026-10-03). It is the **unpatched** submodule; Carafe's
  patches are applied at build time into `runtime/build/wine-switch/carafe-patched/`. No Carafe patch changes the
  memory functions cited here (`grep` of `runtime/patches/autorun/*.patch` for `create_backing`, `protect_range_locked`,
  `horizon_pages_`, `map_backing_at`, `set_protection`, `native_heap`, `horizon_get_memory_info`, `horizon_memfile`
  finds only context lines in 0003 and 0128), so the line numbers hold for the shipped code.
- `dxvk/...` is `carafe/runtime/build/dxvk-src/src` (DXVK 3.1.1, `b1a1c99`).
- `loader/...` is `carafe/runtime/loader/source`, `overlay/...` is `carafe/runtime/overlay`.
- mesa-switch (the console's Mesa) is not on disk. Its facts come from
  [../../orchestrator/findings/shared-memory-attributes.md](../../orchestrator/findings/shared-memory-attributes.md),
  which read it on GitHub at `d4a00ea0`; upstream Mesa in `carafe-lab/external/mesa` is used only as a proxy and is
  marked so.
- **Hypothesis** marks anything not proven by code.

## 1. Summary

**One pool backs everything.** Carafe's loader takes the process's whole free memory as one heap at start-up
(`svcSetHeapSize` of TotalMemorySize − UsedMemorySize − 2 MB − 96 MB, `loader/main.c:17, 115-138`). It copies the
runtime NRO into the bottom of that heap and passes the remainder to the runtime as its libnx heap
(`loader/main.c:345-361, 426-456`). Horizon has no demand paging: the heap is physical memory from that moment, and
`InfoType_UsedMemorySize` counts all of it (`autorun/dlls/ntdll/unix/horizon.c:3906-3909`). Everything else is carved
out of that heap by newlib `malloc`: Wine's guest memory, file views, thread stacks, FEX's translated code, Vulkan/GPU
memory, caches. The log's `heap_used 3030 MB / free 52 MB` is that heap (≈3082 MB). TotalMemorySize (3285 MB in
`[INIT]`) also covers the 96 MB the loader keeps out of the heap and the ≈88 MB runtime image.

**The heap is two allocators that cannot lend to each other.** Autorun wraps newlib's malloc
(`autorun/horizon-wine/CMakeLists.txt:855-857`, `autorun/horizon-wine/source/native_heap.c`). Plain `malloc` grows up
from the bottom by `sbrk`. Page-aligned requests of 64 KB or more (`memalign`/`aligned_alloc`, alignment ≥ 4096)
come from a 64 KB-unit "backing heap" that grows down from the top (`native_heap.c:113-133`,
`backing_heap.h:7, 60-104`). `sbrk` may not pass the backing heap's bottom (`native_heap.c:90-105`), and a backing
allocation fails when no single free run is large enough (`backing_heap.h:95-98`). Free newlib chunks are never used
for page-aligned memory, and free backing holes are never used for `malloc`. `heap_free_mb` adds both kinds of free
memory together (`autorun/horizon-wine/source/runtime.c:1320-1325` with the wrapped `mallinfo`, `native_heap.c:186-199`).
So "52 MB free" does not mean a 4 MB block exists, or a 1 MB `malloc` will succeed.

**Layout (39-bit process, NPDM `address_space_type: 3`, `runtime/build/npdm/carafe-runtime.json:12`):**

- *Heap region*: the libnx heap, i.e. all physical memory (above).
- *ASLR region*: Wine's whole address space (`horizon.c:14074-14095`). The kernel's heap and alias regions are kept
  out of Wine's free-range search (`virtual.c:4560-4601`), and the low 4 GB are reserved early for the guest
  (`virtual.c:4391-4462`). Addresses here cost nothing physical.
- *Alias region*: refused by `svcMapProcessCodeMemory`; Carafe's 0003 makes Wine's search skip it. Not a memory
  consumer.
- *Stack region*: libnx thread stacks, and a window at its top for native mappings (stacks, FEX code arenas, section
  anchors): 512 MB, or 768 MB/5⁄8 on a 4 GB space (`virtual.c:4343-4346, 4414-4439`). libnx takes stacks from the heap
  and maps them there (`horizon.c:18479-18485`).
- *Guest pages*: every Wine view that has memory is heap pages aliased into the ASLR region by
  `svcMapProcessCodeMemory(addr, heap_source, size)` (`horizon.c:17451`). The source pages are lent, so the memory is
  counted once, in the heap.

**Reserve vs commit (proven):**

- A reserve (`MEM_RESERVE`, or any `PROT_NONE` anonymous mmap) costs a libnx `VirtmemReservation` and a mapping slot,
  no heap pages (`horizon.c:19483-19524`; `horizon_mmap_fixed` 19780-19787).
- A commit takes heap memory at once and zeroes it (`create_backing_locked`, `horizon.c:18095, 18105`). Committing
  inside a reservation maps the whole 64 KB-aligned chunk around the range (`horizon.c:19404-19440`,
  `horizon_mman.h` `HORIZON_COMMIT_CHUNK`).
- Memory is backed on first touch only for anonymous mappings of 256 MB or more made with access
  (`HORIZON_LAZY_MAPPING_MIN`, `horizon.c:19526-19538, 19788, 19894, 19941`): a fault commits a 2 MB piece
  (`horizon_commit_lazy_fault`, `horizon.c:19593-19651`, called from the exception entry at 16804).
- `MEM_RESET`/`madvise` does nothing (`horizon.c:20109-20113`; `virtual.c:6414`).

**File mappings (proven):**

- *Whole range copied*: a view of a file section is a fresh heap block the size of the view, zeroed, then filled by
  reading the file (`map_file_into_view` → `mmap(fd)`, `virtual.c:2879-2916` → `horizon_mmap_fixed` →
  `map_backing_at` → `create_backing_locked` → `read_fd_at`, `horizon.c:18105-18117, 17172-17224`). The view is one
  allocation; unlike anonymous memory it is not split into chunks, so it needs one contiguous free run.
- *Not shared*: each `mmap` of a file makes its own backing, so two views of one file section are two copies. Writes
  are not shared between them. A shared writable view is written back to the file only when its backing is freed
  (`horizon.c:18119-18131, 17293-17294`).
- *Sections without a file* (`CreateFileMapping(INVALID_HANDLE_VALUE)`): the whole section size is allocated and
  zeroed when it is created, even for `SEC_RESERVE` (`horizon_memfile.h:112-140`, `horizon.c:4121`). Its views are
  shared: they alias the section's own pages through anchors (`horizon_memfile.h:1-17`, `horizon.c:19971-20038`).
  This needs syscalls 0x74/0x75, which Carafe's NPDM grants (`carafe-runtime.json:155-156`); without them every view
  would be a copy (`horizon.c:20048-20060`).
- *Images (EXE/DLL)*: copied the same way as file views: header and sections are read into committed memory
  (`virtual.c:3605-3870`, `map_pe_header` / `map_file_into_view`). NLS files (`sortdefault.nls` 3.4 MB, `locale.nls`
  0.8 MB, a few code pages, `stage/switch/wine/share/wine/nls`) are file sections too, so they are copies.
- *Unmap*: a backing goes back to the heap only when its last mapping is unmapped (`release_backing`, `horizon.c:17309-
  17321`; `free_backing` 17275-17307). A partial unmap or decommit only splits the bookkeeping: the pieces still hold
  the whole backing (`split_backing_mapping`, `horizon.c:18360-18417`). Decommit is `mmap(PROT_NONE, MAP_FIXED)`
  (`virtual.c:3002-3017`), which takes that path. A later recommit of the same pages makes a new backing while the old
  one is still held. Pages still lent as an alias source are leaked on purpose (`horizon.c:17280-17292`). Pool
  arenas (2 MB) are freed only when empty, and up to 8 empty arenas per pool are kept (`horizon_pool.h:41-47, 141-171`).
  `horizon_pages_trim` runs only in the swap experiment (`horizon.c:17732, 17760, 17778`).

**Why the game got `c0000022`:** Wine's commit path collapses every host failure to `STATUS_ACCESS_DENIED`
(`set_protection`, `virtual.c:2529`); the `ENOMEM` from `create_backing` is lost in a `BOOL`. Section 3 has the full
chain. The same shortage met by a single `MEM_RESERVE|MEM_COMMIT` call returns `STATUS_NO_MEMORY` (`virtual.c:2752-2756`,
`2853`). Nothing returns `STATUS_COMMITMENT_LIMIT`.

**FEX:** translated code lives in kernel code-memory arenas whose backing is `aligned_alloc` from the heap
(`autorun/horizon-wine/source/fex_jit.c:17-20, 51-102`). The cache may grow to **512 MB for a 64-bit guest**, 256 MB
for a 32-bit one (`fex/cache_policy.h:18-20`), in 16–64 MB chunks (`fex/code_storage.h:108-110`). Chunks are kept
until the whole cache generation is dropped (`code_storage.h:30-33, 75`). The lookup caches are small:
`FEX_DISABLEL2CACHE` defaults to on (L2 off), dynamic L1 defaults to on, L1 holds at most 64 K entries (1 MB), the L2
backing is 4 MB (`fex_options.h:60-65`, `cache_policy.h:22-28`). `code_mb` in `[PROGRESS]` is **Box64's** dynarec and
says nothing about FEX (`runtime.c:1227-1262`, inside `#ifdef WINE_NX_BOX64_DYNAREC`). FEX's memory is logged nowhere.

**GPU:** NVK's device memory is `memalign` heap memory plus `nvMapCreate` (prior finding,
`nouveau_horizon_memory.c:590-606`). So "VRAM" is the same 3 GB heap. NVK reports one 1.5 GiB device-local heap, and
DXVK passes it on as `DedicatedVideoMemory` = 1536 MB and, since there is no system heap, `SharedSystemMemory` = 1536 MB
too (`dxvk/dxgi/dxgi_adapter.cpp:456-462, 477-480, 504-506`). Games are told 3285 MB of RAM
(`GlobalMemoryStatusEx`) plus 1.5 GB of VRAM, while both come out of one ≈3 GB heap. DXVK enforces no budget on a
UMA device (`dxvk/dxvk/dxvk_memory.cpp:754-755`). It keeps empty chunks up to one max chunk (device pool) or four
(mapped pool) per memory type, for up to 20 s (`dxvk_memory.cpp:1743-1792`). Nothing tells it that Wine's side is
short.

**Swap experiment:** `WINE_NX_SWAP_POC` pages private anonymous guest memory out to an SD file under pressure. It is
off in Autorun by default (`CMakeLists.txt:860`) and off in Carafe's build (`runtime/build/wine-switch/CMakeCache.txt:344`).
It needs Autorun's launcher to create the swap file and a `swap-mb` key in `launcher.txt` (`runtime.c:3971-3993`).
It is not usable in Carafe without a rebuild and work on how the NSP gets its swap file.

## 2. Consumers

Sizes are proven when they come from code or the built ELF; "scales with" marks behaviour, not measured shares.

| # | Consumer | Where in code | How it scales | Freed when | In the log today |
|---|---|---|---|---|---|
| 1 | Runtime NRO image: text 26.5 MB, ro 13.3 MB, data 1.1 MB, **bss 44.6 MB** = 87.6 MB | `loader/main.c:345-361, 413-427` (copied into heap bottom); sizes from the header of `runtime/build/wine-switch/wine-nx-runtime.nro` (Nov 5 build) | Fixed. Largest bss items (from `wine-nx-runtime.elf` symbols): Box64 `static_insts` 11.25 MB (`vendor/box64/src/dynarec/dynarec_native.c:343`), FFmpeg `ff_tx_tab_*` 16.3 MB, profiler `targets` 2 MB (`thread_profile.c:105`), `g_fatCache` 2 MB, native-heap `blocks` 1.5 MB (`native_heap.c:22`), `mapping_slots` 0.88 MB | Never | No |
| 2 | Loader's held-back 96 MB (`HEAP_RESERVE`) | `loader/main.c:17, 128-130` | Fixed | Never; not part of the libnx heap | No |
| 3 | Committed anonymous guest memory (`VirtualAlloc`, Wine/PE heaps, game allocators) | Commit: `virtual.c:6421` → `horizon.c:19348-19468` → `18241` → `18076-18135`. Reserve+commit in one call: `horizon.c:19798-19806`, chunks of 16 MB halving to 2 MB on ENOMEM (`19314-19346`) | Eager, zeroed, by 64 KB chunk. ≥ 256 MB mappings are lazy, 2 MB per first touch. Each chunk ≤ 2 MB comes from a 2 MB pool arena, bigger ones from a dedicated 2 MB-aligned block rounded up to 2 MB (`horizon_pool.h:117-138`) | When every page of the backing is unmapped (`horizon.c:17309-17321`) | Only failures (`[HMAP] … failed`, `[VAFAIL]`); `[VA]` per call only in verbose runs |
| 4 | Backings kept alive by partly unmapped/decommitted views | `split_backing_mapping`, `horizon.c:18360-18417`; decommit `virtual.c:3013` | Grows with decommit/recommit churn inside one backing (≤ 16 MB per chunk) | When the last piece goes | No |
| 5 | Page-pool arenas (2 MB) for backings and section pages; up to 8 empty kept per pool | `horizon_pool.h:41-47, 64-115, 141-171`; pools `horizon.c:3609-3644` | Fragmentation: one live 64 KB page keeps its 2 MB arena | Arena empty and more than 8 active | `[MEMPOOL] arena_mb/arena_peak_mb` |
| 6 | File views (`MapViewOfFile` of a real file), including Mono assemblies or game archives if mapped | `virtual.c:2879-2916` → `horizon.c:18105-18131` | Full view size per view, copied at map time; one contiguous block per view | Last unmap of that view (write-back first if shared+writable) | No |
| 7 | PE images (EXE, DLLs) and NLS | `virtual.c:3605-3870`; NLS files 3.4 + 0.8 MB + code pages | Image virtual size per loaded module | `FreeLibrary` / exit | `[MI]`/`[IV]` lines only in verbose runs |
| 8 | Sections without a file (pagefile-backed; DXVK, Unity, IPC) | `horizon_memfile.h:112-140`; views `horizon.c:19971-20038` | Whole section size at creation, even `SEC_RESERVE`; views share it | Last handle and view gone (`horizon_memfile.h:148-163`); anchored pages the kernel would not release are kept | `[MEMPOOL] anchors/anchor_mb/section_views` (count only) |
| 9 | Guest thread stacks | `virtual.c:5428-5520` | Reserve = PE stack reserve (≥ 1 MB); **commit ≥ 1 MB at once** (`5450`) | Thread exit | No |
| 10 | Per-thread Wine data and native stacks | `virtual.c:5163-5185` (64 KB thread data); ARM64EC emulator stack 256 KB committed (`thread.c:1283-1286`); native pthread stack 1 MB (`thread.c:1384`, `unix_private.h:192`) from the heap (`horizon.c:18479-18485`) | ≈ 2.3 MB + the guest stack's commit per Windows thread (sum of the rows above) | Thread joined; libnx returns a stack only at join, and the last ended thread waits for the next (`horizon.c:18479-18485`) | `[BUDGET]` counts threads, not bytes |
| 11 | FEX translated code | `fex_jit.c:17-20, 51-102, 109-180, 214-260`; `fex/code_storage.h`; limit `cache_policy.h:18-20` | 16 → 128 MB arenas (doubling), chunks 16–64 MB, up to 512 MB (64-bit guest) / 256 MB (32-bit) | Arena when its last mapping closes; chunks only when the whole cache generation is unused (`code_storage.h:30-33, 75`) | No (`code_mb` is Box64) |
| 12 | FEX lookup caches | `fex/horizon.patch:445-470, 540-575`; `cache_policy.h:14-28` | Page-pointer table for 2^28 guest bytes (512 KB) + L1 ≤ 64 K × 16 B + L2 4 MB (L2 off by default). Allocated through FEX's `VirtualAlloc` without commit, i.e. Wine views | Cache clear / generation end | No |
| 13 | FEX per-thread state and compile buffers | Thread state in Wine virtual memory (`runtime/patches/fex/0023`); decoder buffer = MaxInst + 64 entries, two IR lists of 1–8 MB each (`runtime/patches/fex/0003`, header) | Per thread that compiles | Thread exit | No |
| 14 | FEX's own heap (`malloc` inside FEX: IR, block maps, containers) | `fex/horizon.patch:1307-1405` → unix call → `fex_unix.c:106-126` (`malloc`/`calloc`/`realloc`) | Grows with translated code; newlib for small, backing heap for ≥ 64 KB aligned | `free` | No |
| 15 | Vulkan device memory (DXVK → NVK) | NVK `memalign` + `nvMapCreate` (prior finding); win32u wrapper `autorun/dlls/win32u/vulkan.c:1300-1547` | DXVK chunks: 128 MB device pool, 64 MB mapped pool on a 1.5 GiB heap (`dxvk_memory.h:1128-1129`, `dxvk_memory.cpp:1978-1995`); empty chunks kept up to 1×/4× max chunk per type, freed after 20 s (`dxvk_memory.cpp:1743-1792`) | DXVK frees chunks; NVK may keep backings in its cache (row 16) | `[NXVK] memory: … MB live` every 256 allocations (`vulkan.c:373-389`); failures |
| 16 | NVK backing cache ("whole backings" kept to avoid cache-attribute changes) | mesa-switch `nouveau_horizon_memory.c:269-271` (prior finding) | **Hypothesis:** bounded by Mesa's own policy, unknown here | Mesa's eviction | GL path only: `bo_new/bo_reuse/bo_evict` (`runtime.c:1290-1305`) |
| 17 | Vulkan host-import fallback | `vulkan.c:1426-1442` | When a native allocation fails with `OUT_OF_HOST_MEMORY` on 64-bit, the same size is retried as a Wine view (`NtAllocateVirtualMemory`, `vulkan.c:497`), i.e. rows 3/5 | `vkFreeMemory` | First 8 imports logged |
| 18 | SD read cache | `autorun/horizon-wine/source/sd_cache.c:103-157`; `sd_read_cache.h:38-44` | 32 MB minimum, up to 192 MB: a third of the free heap above a 512 MB floor, resized at most every 5 s on a read | Shrinks only on that resize; `wine_nx_sd_cache_reclaim` is called only by the swap experiment (`swap_native.c:301`) | `[PROGRESS] cache_mb` |
| 19 | Mapping bookkeeping | `horizon.c:3638-3641` (4096 backing + 8192 mapping slots static); beyond that `calloc` per object (`horizon_pool.h:16-24`) | Per mapping/backing | Freed per object | `[MEMPOOL] backing_slots/mapping_slots` (see gaps) |
| 20 | Wine server state: registry tree, objects, session | `horizon_registry.h:1-19, 219-272, 620-660` (newlib `malloc`) | Size of `system.reg`/`user.reg` and keys created | Process end | No |
| 21 | Carafe overlay: loading-screen movie | `overlay/carafe_loading.c:339, 1320-1386, 1421` | width × height × frames (≤ 256 frames, ≤ 1024×1024) | **Never freed** (no `free` of `g_movie` frames) | `trace("%s %dx%d, %d frames")` |
| 22 | Carafe overlay: settings menu file | `overlay/carafe_settings.c:17, 1069`; `overlay/carafe_overlay.c:1640` | One `malloc(1 MB)` when the menu loads, then `realloc` to the file size | After parsing | `[CARAFE] settings menu: …` on failure |
| 23 | Carafe overlay: error screen, background, HID | `carafe_error.c:1486-1647` (1280×720×4 picture, 64 KB buffers), `carafe_draw.c:162`, `carafe_hid.c:456-500` | Small, on demand | After use | No |
| 24 | Kernel side, not the heap: page tables and kernel objects (`system_resource_size` 16 MB, `carafe-runtime.json:263`); 20000 memory blocks for all applications (`horizon_mman.h` comment) | — | Per mapping | — | `[HMAP] map_code failed … rc=` |

Two structural effects are not a consumer of their own, but they decide whether a request succeeds:

- **Two pools (newlib below, backing heap above).** A 1 MB `malloc` (row 22) fails once `sbrk` meets the backing
  heap, even if the backing heap has free holes. A 4 MB commit (row 3) fails if no 4 MB run is free in the backing
  heap, even if newlib has megabytes free (`native_heap.c:90-133`).
- **Commit inside a reservation is one block.** `protect_range_locked` commits the whole 64 KB-aligned range with
  one `map_backing_at` (`horizon.c:19432` → `18241`). It never splits the range, while reserve+commit in one call
  retries with smaller chunks (`horizon.c:19330-19337`). A 4 MB commit after a reserve needs one free 4 MB run
  (first a 2 MB-aligned one, then any 64 KB-aligned one; `horizon_pool.h:117-138`).

## 3. From `create_backing` errno 12 to the status the game gets

The `c0000022` in the log comes from a commit into memory the game had reserved before. Each step is in the shipped
code:

1. `NtAllocateVirtualMemory(base, 4 MB, MEM_COMMIT, PAGE_READWRITE)` on a reserved view → `allocate_virtual_memory`,
   "commit the pages" branch → `set_protection` (`virtual.c:6416-6421`).
2. `set_protection` → `set_vprot(view, base, size, vprot | VPROT_COMMITTED)` (`virtual.c:2529`).
3. `set_vprot` writes the new protection bytes **first** (`set_page_vprot`, `virtual.c:2502`) and then calls
   `mprotect_range` (`virtual.c:2504`) → `mprotect_exec` → `mprotect`, which on Horizon is `horizon_mprotect`
   (`virtual.c:139-141`, `horizon.c:20093-20107`).
4. `protect_range_locked` finds a reservation mapping with `PROT_NONE` and commits the 64 KB-aligned chunk
   (`horizon.c:19404-19440`) with `replace_reservation_mapping(…, TRUE)` → `map_backing_at` (`horizon.c:18241`) →
   `map_backing_at_locked` → `create_backing_locked`.
5. `create_backing_locked`: `horizon_pages_alloc_any(4 MB)` → `aligned_alloc(2 MB, 4 MB)`, then
   `aligned_alloc(4096, 4 MB)`, both served by the backing heap (`horizon_pool.h:117-138`, `native_heap.c:113-133`).
   Both fail → `errno = ENOMEM` (`horizon.c:18095-18103`).
6. Traces: `[HMAP] create_backing failed … errno=12` (`horizon.c:18146`), `[HMAP] commit reservation failed …
   errno=12` (`horizon.c:19435`). `horizon_trace` sends `[HMAP] … failed` lines to the runtime log even in quiet
   runs: the first 16 per call site, then powers of two (`horizon.c:13846-13870`).
7. `horizon_mprotect` returns −1. `set_vprot` returns `FALSE`. `set_protection` returns **`STATUS_ACCESS_DENIED`**
   (`virtual.c:2529`). The errno is never looked at. This is upstream Wine's mapping: on Linux a failing `mprotect` is
   a permission problem, and memory is never short at that point.
8. `NtAllocateVirtualMemory` logs `[VAFAIL] … status=0xc0000022` (`virtual.c:6560-6577`). It counts it in the
   ACCESS_DENIED bucket and does not dump the views: `[VA] out of address space` and `wine_nx_log_views` run only for
   `STATUS_NO_MEMORY` (`virtual.c:6460-6478`).
9. Afterwards Wine's page table still says those pages are committed read-write (step 3), but the Horizon mapping is
   still an inaccessible reservation. A touch faults, `horizon_commit_lazy_fault` declines it (reservation with
   `PROT_NONE`, `horizon.c:19611-19621`), and the guest gets **`c0000005`**. **Hypothesis:** the `c0000005` in the log
   is this touch, and the endless spin is the game (Unity's allocator or loader) retrying the commit or the access.
   The code does not show which.

Other paths, for comparison:

- `MEM_RESERVE|MEM_COMMIT` in one call: `map_view` → `map_fixed_area`/`map_reserved_area` → `anon_mmap_fixed` →
  `horizon_mmap_fixed` → `map_anonymous_backings` (16 MB chunks, halving to 2 MB on ENOMEM). On failure ENOMEM →
  **`STATUS_NO_MEMORY` (c0000017)** (`virtual.c:2752-2756`, `2853`), plus `[VA] out of address space`. That message
  is misleading when the heap, not the address space, ran out.
- `svcMapProcessCodeMemory` refusing with ResourceExhausted/OutOfMemory (kernel memory blocks or page tables) also
  becomes ENOMEM (`horizon.c:17462-17469`), so it ends in the same statuses. The trace differs (`[HMAP] map_code
  failed … rc=`).
- `STATUS_COMMITMENT_LIMIT` (c000012d) is produced nowhere on this path. Windows returns it (or
  `STATUS_NO_MEMORY`) when commit charge runs out.

Where to change the status, if wanted: `set_protection`/`set_vprot` (`virtual.c:2486-2530`) is the single point. The
errno is available right after `mprotect_range` at `virtual.c:2504`. Choosing the status is for another finding to
decide.

## 4. Existing diagnostics and what they miss

What each line counts today:

- `[PROGRESS] heap_used_mb / heap_free_mb` (`runtime.c:1316-1347`). `used` = wrapped `mallinfo().uordblks` = newlib
  used + backing-heap used (`native_heap.c:186-199`). `free` = newlib free chunks + backing free holes + the gap
  between them. Empty pool arenas, retained DXVK chunks and the SD cache all count as "used". It does not say how
  much of "free" is newlib-only, how big the largest backing run is, or whether the gap is closed.
  `wine_nx_native_heap_stats` (`native_heap.c:252-270`) computes exactly that (reserved, used, largest, gap, holes,
  small_free, small_largest), but only the swap experiment calls it (`swap_native.c:270-332`).
- `[PROGRESS] cache_mb`: the SD read cache (row 18). `code_mb`, `code_all_mb`, `purged`: Box64 only.
- `[MEMPOOL]` (`horizon.c:3795-3812`): `arena_mb` and `arena_peak_mb` are live/peak 2 MB arenas of both pools;
  `pooled_allocs` (`hits`), `block_allocs` (dedicated blocks) and `shared_allocs` (fallback blocks) are
  **counts since start, not bytes**. `backing_slots` and `mapping_slots` are `pool->used`, which only grows
  (`horizon_pool.h:21`), so they are **high-water marks of the static slots**, not live objects; past 4096/8192 the
  objects are `calloc`'d and not counted. `section_views` is cumulative (`horizon.c:20031`). `anchors`/`anchor_mb`
  are live (`horizon.c:19074-19093`).
- `[MEM] after …` (`runtime.c:2683-2703`): only memory lent away (borrowed/IPC/device-mapped) at start-up steps.
- `[BUDGET]` (patches 0019/0020/0023): threads against the kernel limit, not memory.
- `[VA]`: per-call lines only in verbose runs (`horizon_trace` → `horizon-trace.log`); `[VAFAIL]` sampled per status
  bucket; `[VA] out of address space` + view dump only for `STATUS_NO_MEMORY`.
- `[HMAP] … failed`: size, protection and errno, but not who asked and not the heap's largest free run.
- `[NXVK] memory: … MB live` (`vulkan.c:373-389`): DXVK's live device memory, every 256 allocations; for a game that
  allocates in big chunks this line may appear rarely.
- `[INIT] processors=… memory=… used=…` (`runtime.c:4053-4058`): TotalMemorySize and the adjusted used size.

What is missing to split the 3 GB by consumer:

1. Bytes per consumer on the backing heap and newlib: guest anonymous commits, file-view copies, images, sections
   without a file, guest stacks, native stacks, FEX code arenas, FEX heap, NVK/Mesa, SD cache, overlay. Most of these
   allocate in one place each (`create_backing_locked`, `section_pages_alloc`, `fex_jit.c:74`, `fex_unix.c:106`,
   Mesa's `memalign` via the wrapper), so a per-call-site counter is cheap.
2. The largest free backing run, the gap, and newlib's largest free chunk, in `[PROGRESS]` and at every `[HMAP] …
   failed`. Without them "52 MB free" cannot be told apart from fragmentation.
3. Live backing count and bytes (instead of the `backing_slots` high-water mark), and bytes held by partly unmapped
   backings (row 4).
4. FEX code bytes (arena total and used) and FEX heap bytes.
5. A Wine view summary (committed bytes by kind: valloc, image, file, section, stack) on the first allocation failure
   of **any** status, not only `STATUS_NO_MEMORY`. `wine_nx_log_views` exists (`virtual.c:6476`).
6. Whether NVK exposes `VK_EXT_memory_budget` and what budget DXVK passes to `QueryVideoMemoryInfo`.

## 5. What games are told

- `GlobalMemoryStatusEx` (`autorun/dlls/kernelbase/memory.c:1433-1443`):
  - `ullTotalPhys` = `MmNumberOfPhysicalPages` × 4 KB = TotalMemorySize (`virtual.c:4628-4635, 4663-4664`;
    `horizon.c:3903`), i.e. 3285 MB. That is about 200 MB more than the heap (96 MB held back plus the runtime image).
  - `ullAvailPhys` = the heap's free memory as `heap_free_mb` computes it (`horizon.c:3912-3923`; `system.c:2769-2789`).
    It includes free memory no large request can use (section 1).
  - `ullTotalPageFile` = total + 4 KB; `ullAvailPageFile` ≈ `ullAvailPhys` (`system.c:2782-2789`). There is no swap.
- `SYSTEM_BASIC_INFORMATION.NumberOfProcessors` = 3 (core 3 is kept for graphics workers, `horizon.c:3865-3893`).
- DXGI (DXVK): `DedicatedVideoMemory` = NVK's device-local heap, 1.5 GiB on the console (`Heap 0: 1.50 GiB
  (DEVICE_LOCAL)` in an earlier console log, prior finding). `SharedSystemMemory` = the same 1.5 GiB, because NVK
  reports no non-device-local heap (`dxgi_adapter.cpp:456-462`). No 32 MB adjustment applies (1.5 GiB is not a
  multiple of 4 GiB, `:477-480`); a 64-bit game gets no 3 GB clamp (`:492-498`).
- `QueryVideoMemoryInfo`: `Budget` = NVK's `heapBudget` if `VK_EXT_memory_budget` is present, else the heap size
  (`dxvk/dxvk/dxvk_adapter.cpp:103-136`, `dxgi_adapter.cpp:296-312`).
  **Hypothesis (upstream proxy):** if mesa-switch keeps upstream NVK's code, the heap size is 3⁄4 of
  `os_get_total_physical_memory` (1.5 GiB = 3⁄4 × 2 GiB). The budget is then 90 % of min(3⁄4 × available + used, heap
  size) (`carafe-lab/external/mesa/src/nouveau/vulkan/nvk_physical_device.c:1296-1316, 1576-1606`). "Available" would
  come from Autorun's wrapped `os_get_available_system_memory` = heap capacity − (backing used + small used)
  (`native_heap.c:202-212`), so the budget would fall as the heap fills.

Games size caches from these numbers. Unity reads total RAM and VRAM. **Hypothesis:** a Unity game that treats 3285 MB
RAM and 1536 MB VRAM as separate pools plans for about 4.8 GB on a ≈3.0 GB heap. Its readable textures and meshes
would then exist twice (CPU copy in Wine views, GPU copy in NVK memory), both from the same heap.

## 6. Swap experiment (`WINE_NX_SWAP_POC`)

- What it does: private, non-executable anonymous backings of tracked `VirtualAlloc` views are written to an SD swap
  file. Their pages are unmapped and freed, and a later fault restores them (`horizon.c:17515-17640, 17676-17734,
  19540-19591`; `horizon_swap.h:25-70`). It triggers when the estimated free heap drops below the request + 128 MB
  (`swap_pressure_locked`, `horizon.c:17736-17765`). It also hooks Vulkan allocation retries (`vulkan.c:1400-1458`)
  and the SD cache (`swap_native.c:247-301`). Stacks, TEBs, guard and write-combined pages are excluded
  (`virtual.c:5031-5032, 5085-5087, 6440-6449`). With swap on, anonymous chunks are capped at 2 MB
  (`horizon.c:19322-19325`).
- State: a CMake option, off by default ("experimental SD-backed guest memory paging", `CMakeLists.txt:860-878`).
  It requires FEX. It is off in Carafe's runtime build (`CMakeCache.txt:344`). It starts only with `swap-mb` in
  `launcher.txt` and a prepared `swap-poc` file on a 39-bit forwarder (`runtime.c:3971-3993`). The file is created by
  Autorun's launcher UI (`launcher_swap.c`). Host tests exist (`check-swap.sh`); no console result is recorded in the
  repository.
- Usable for Carafe today: no. It needs a rebuild with the option on, a way for the NSP to create or ship the swap
  file, and console proof. Even then it pages only private anonymous guest memory. It does not page file-view copies,
  sections, FEX code, NVK memory or the overlay.

## 7. Open questions

1. **Breakdown of the 3030 MB** in the Cat Mail Co run. No counter in the code splits it. The `[NXVK] memory … MB live`
   lines of that run, if any, give the DXVK share; the rest needs the counters in section 4.
2. **Fragmentation or exhaustion?** Was there a free 4 MB run when the commit failed? It needs the largest free run
   (`wine_nx_native_heap_stats`) at the failure.
3. **mesa-switch:** where does 1.5 GiB come from? Does NVK expose `VK_EXT_memory_budget`? How large can its backing
   cache grow (row 16)? The source is not on disk.
4. **The 96 MB `HEAP_RESERVE`:** what uses it at run time (TLS pages, kernel objects, loader leftovers)? How much
   could the heap get back?
5. **Unity's pattern:** does it reserve and then commit in 4 MB steps, and decommit and recommit? If so, row 4
   (backings held by decommitted pieces) may grow. How does it react to `c0000022` vs `c0000017`?
6. **FEX code generations:** when the 512 MB limit or a cache clear starts a new generation, is the old one held until
   every thread leaves it? If so, the code cache can briefly take twice its size (`code_storage.h:30-33, 75`).
7. **FEX compile buffers and the lookup cache:** they sit in Wine views. Are they committed in full on Horizon (eager
   commit), or only the pages FEX's overcommit tracker commits? Size per compiling thread on the console.
8. **Kernel limits:** can the 16 MB system resource (page tables for about 3 GB aliased twice) or the 20000 memory
   blocks run out before the heap does? Both show up as ENOMEM through `map_code` (section 3).
9. **Bss that could go:** the 11.25 MB Box64 instruction buffer and the 16.3 MB of FFmpeg tables are in every Carafe
   run, even when Box64 does not run. Are they needed?
