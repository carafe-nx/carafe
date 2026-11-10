# Horizon OS memory model for a Carafe title

Research snapshot: 2026-11-05. For subsequent runtime changes and validation, see the
[report](../REPORT.md#updates-after-the-study).

Scope: how memory works on Horizon OS for an application like Carafe's, and what the platform allows.
Read-only study, 2026-11-05. Sources: Atmosphère `master` (Mesosphère kernel, `pm`), libnx `master`,
`danfromtico/mesa-switch@d4a00ea0ab` (the Mesa the console runs), Eden `carafe-lab/external/eden` (HLE kernel, used as
a readable reference for SVC semantics), Autorun `carafe/external/autorun`, Carafe `carafe/runtime`, and the console
logs in `console-logs/catmail-2026-11-05`.

Every claim is marked **[fact]** with a source, or **[hypothesis]**.

## Summary

- A retail 4 GB Switch (Erista and Mariko alike) gives the **application pool 3285 MB**; the applet pool gets 507 MB
  (less Atmosphère's share) and the system pool the rest. 6 GB (dev units) and 8 GB (modded, Atmosphère 1.11.0+ with
  the memory-mode flag) arrangements give the application 4916 MB and 6964 MB. Neither the NPDM nor the NACP can enlarge
  the pool; only the kernel's memory arrangement (secure monitor + Mesosphère) decides it.
- Carafe's NSP is an Application with a 39-bit address space and a 16 MB system resource. On the console the loader
  takes **3168 MB as one heap with `svcSetHeapSize`**; the kernel reports `TotalMemorySize` 3189 MB. Everything the
  runtime uses — Wine's guest memory, FEX, DXVK, NVK's GPU buffers, the SD cache — is carved out of that one heap by
  user-mode allocators. Freed memory goes back to those allocators, never to the kernel.
- Horizon has **no demand paging, no swap, no file-backed mappings, no overcommit**: every mapped page is physically
  allocated at map time, and files live in a user-mode service (`fs`) reached by IPC.
- The platform does allow a process to **give physical pages back and take them again**: `svcSetHeapSize` can shrink
  the top of the heap, and `svcMapPhysicalMemory`/`svcUnmapPhysicalMemory` map and free memory at 4 KB granularity in
  the alias region (Carafe's NPDM already has the system resource this needs; the alias region is 64 GB and unused).
- The platform allows **user-mode paging**: data and instruction aborts (translation and permission faults) reach the
  process's own exception handler with ESR and FAR, and the handler can fix the mapping and resume. Autorun already
  does this for lazy commit, and has an experimental SD-backed pager (`WINE_NX_SWAP_POC`, off by default).
- Limits for such paging: the kernel serialises user exceptions per process; faults the kernel takes on our behalf
  (IPC buffers, SVC arguments) and GPU/SMMU accesses never reach the handler, so those pages must stay resident.
- The GPU shares the same pool and the same heap: NVK allocates every buffer with `memalign` from the newlib heap and
  registers it with nvmap. NVK advertises a heap of 3/4 of `TotalMemorySize` (2.33 GiB on Cat Mail Co).
- The SD card on Horizon runs at UHS-I SDR104: about 70–97 MB/s sequential read on good cards, and Carafe's own
  console measurements show 37–48 MB/s with 0.83 ms per request through `fsFile`.

## 1. Memory pools and arrangements

### 1.1 Pool sizes [fact]

Mesosphère `libraries/libmesosphere/source/board/nintendo/nx/kern_k_system_control.cpp`
(`KSystemControl::Init::GetApplicationPoolSize` / `GetAppletPoolSize`):

| Memory arrangement | Application pool | Applet pool (base) |
|---|---|---|
| `MemoryArrangement_4GB` (retail, default) | **3285 MB** | 507 MB |
| `4GBForAppletDev` | 2048 MB | 1554 MB |
| `4GBForSystemDev` | 3285 MB | 448 MB |
| `6GB` (dev kits) | 4916 MB | 562 MB |
| `6GBForAppletDev` | 3285 MB | 2193 MB |
| `8GB` (RAM mod) | 6964 MB | 562 MB |

- The applet pool loses an extra share to Atmosphère's custom system modules: 40 MB on firmware 23.0.0+, 7 MB on
  21.x, 14 MB on 20.x, 40 MB before (same function), and `pm` takes 32 MB more from the applet pool on 3.0.0+
  (`stratosphere/pm/source/impl/pm_spec.cpp`, `ExtraSystemMemorySizeAtmosphere = 32_MB`).
- The system pool is the remainder of DRAM, with a floor (`GetMinimumNonSecureSystemPoolSize`).
- The arrangement comes from the secure monitor: `smc::GetConfig(ConfigItem::MemoryMode)`, values `0x01` 4 GB,
  `0x11` 6 GB, `0x21` 8 GB (same file, `GetMemoryArrangeForInit`). Erista and Mariko retail units are both 4 GB and
  both report `0x01`; the SoC revision does not change the split.

### 1.2 Can anyone change it?

- **8 GB / 6 GB mods [fact]:** Atmosphère 1.11.0 added "support for memory modes above 4GB"
  ([changelog](https://github.com/Atmosphere-NX/Atmosphere/blob/master/docs/changelog.md)); PR
  [#2748](https://github.com/Atmosphere-NX/Atmosphere/pull/2748) lets exosphère use the memory mode on retail units
  when a boot flag is set (hekate's option, per hekate's
  [release notes](https://github.com/ctcaer/hekate/releases)). A modded 8 GB console therefore gives the application
  6964 MB. The process can map that: the 39-bit heap region is 8 GB (log line `[MAP] start-up: heap region
  42e2e00000+8388608KB`) and `MainMemorySizeMax` is 8 GiB (`eden/src/core/hle/kernel/k_memory_layout.h:28`).
- **NPDM [fact]:** the NPDM chooses the pool (`pool_partition`: Application, Applet, SecureSystem, NonSecureSystem —
  [switchbrew NPDM](https://switchbrew.org/wiki/NPDM)), not its size. Carafe uses `pool_partition: 0` = Application
  (`carafe/runtime/loader/carafe-loader.json:10`). Every other pool is smaller.
- **`system_resource_size` [fact]:** memory reserved for the process's own page tables and kernel memory blocks, up to
  0x1FE00000 (switchbrew NPDM). It is charged against the same application resource limit, so it does not add memory;
  it is what `svcMapPhysicalMemory` requires (section 2.2). Carafe sets 16 MB (`carafe/runtime/Makefile:117`).
- **NACP [fact]:** no NACP field sizes the application pool. The only memory field is `JitConfiguration`
  (offset 0x33B0: flags + `MemorySize`), memory for the `jit` service
  ([switchbrew NACP](https://switchbrew.org/wiki/NACP)). Which pool that memory comes from is not documented
  there (open question 3).
- **Resource limits [fact]:** `pm` sets the Application group's physical memory limit to the application pool size
  (`pm_spec.cpp`, `svc::GetSystemInfo(...Application)`), thread count 96 (matches `[THREAD] create failed
  rc=0x10801 threads=96/96` in the Cat Mail Co log), transfer memories 32.
  **[hypothesis]** Raising our own limit with `svcSetResourceLimitLimitValue` (allowed by our NPDM) would not help:
  pages come from the pool's page heap, and the application pool has no more pages than its size.
- **Custom kernel [hypothesis]:** a patched Mesosphère could move memory from the applet pool to the application pool
  (at most a few hundred MB from the 507 MB applet pool, at the cost of HOME-menu/library-applet stability). Autorun
  already ships optional Mesosphère patches for other reasons (`external/autorun/horizon-wine/mesosphere/*.patch`),
  so this path exists technically, but it would only help users who install it.
- **Atmosphère steals from the application pool [fact]:** since 1.5.4 `ams.mitm`/`pm` can take memory from the
  application pool when building a LayeredFS RomFS for mods (changelog). This does not apply to Carafe's NSPs unless a
  user puts a mod for the title into `atmosphere/contents`.

### 1.3 What the NSP gets now [fact unless marked]

- NPDM: generated from `runtime/loader/carafe-loader.json` with `address_space_type` 3 (39-bit) or 2 (32-bit) and
  `system_resource_size` 0x1000000 (`carafe/runtime/Makefile:113-118`); Application type, all SVCs 0x00–0xBF
  including privileged ones (`svcGetSystemInfo` 0x6F, `svcCreateCodeMemory` 0x4B, `svcMapPhysicalMemory` 0x2C,
  `svcSetResourceLimitLimitValue` 0x7E: `carafe-loader.json:83-184`).
- The loader computes `TotalMemorySize − UsedMemorySize − 2 MB`, rounds to 2 MB, keeps **96 MB** out
  (`HEAP_RESERVE 0x6000000`) and calls `svcSetHeapSize` once (`carafe/runtime/loader/source/main.c:17,114-139`). The
  runtime `.nro` is read into the heap and code-mapped from it; the rest of the heap goes to the runtime as its heap
  override (`main.c:345-456`).
- Cat Mail Co console log (`autorun_runtime.log:14-21`): heap mapped 0x42e2e00000–0x43a8e00000 = **3168 MB**;
  `[INIT] processors=3 memory=3189 MB` (`TotalMemorySize`, `horizon.c:3903` via `runtime.c:4057`);
  `[MEM] after the network: 7 regions lent away, 102080 KB` (transfer memory lent to `bsd`/`nvdrv` etc.).
- 32-bit titles (type 2) see `memory=2048 MB` in every log (`console-logs`, `grep memory=`): the kernel caps
  `TotalMemorySize` at the heap region size (`m_max_process_memory = GetAddressSpaceSize(..., Type_Heap)`,
  `kern_k_process.cpp`).
- Unresolved: with `TotalMemorySize` = 3189 MB at runtime start and a 3168 MB heap, the 96 MB the loader kept out of
  the heap appear to be already gone from the free limit (open question 1).

## 2. Memory SVCs and kernel facilities

### 2.1 Overview

| SVC | What it does | Notes for us |
|---|---|---|
| `svcSetHeapSize` (0x01) | Grows or shrinks the one heap, 2 MB units, from a fixed 2 MB-aligned start | Physical pages allocated on grow, freed on shrink. Shrink only works if the freed tail is plain Normal RW memory with no attributes (`eden k_page_table_base.cpp:2130-2141`) |
| `svcSetMemoryPermission` (0x02) | `---` / `r--` / `rw-` on reprotectable memory; no X, no write-only; not on locked or uncached ranges ([switchbrew SVC](https://switchbrew.org/wiki/SVC)) | Used by Autorun for page protections |
| `svcSetMemoryAttribute` (0x03) | Only `Uncached` (17.0.0+: also `PermissionLocked`) (switchbrew SVC) | libnx/NVK use it for coherent GPU memory |
| `svcMapMemory`/`svcUnmapMemory` (0x04/0x05) | Alias a range into the **stack region**; source becomes `---` + borrowed (switchbrew SVC) | Alias only, no new memory |
| `svcQueryMemory` (0x06) | Region type, perms, attributes (borrowed, IPC-mapped, device-mapped, uncached, permission-locked) | Autorun's fault paths rely on it |
| `svcMapSharedMemory` (0x13) | Maps a shared-memory object a service created | Charged to the creator ([hypothesis] for service-created objects) |
| `svcCreateTransferMemory` (0x15) | Lends our own pages to a service; source becomes inaccessible/borrowed | Application limit: 32 objects (`pm_spec.cpp`) |
| `svcMapPhysicalMemory`/`svcUnmapPhysicalMemory` (0x2C/0x2D) | Allocate/free physical pages at a chosen address in the **alias region**, 4 KB granularity | Needs a non-zero system resource; see 2.2 |
| `svcMapPhysicalMemoryUnsafe` (0x48), `svcSetUnsafeLimit` (0x4A) | Map from the Unsafe pool (system pool memory for GPU DMA of system modules) | Must be in the "unsafe alias region"; the process's pool must not be Unsafe (`kern_svc_physical_memory.cpp`). [hypothesis] Not meant for applications; untested |
| `svcCreateCodeMemory`/`svcControlCodeMemory` (0x4B/0x4C) | JIT memory: one RW owner view, one R/RX slave view of the same pages (switchbrew SVC) | Autorun uses it for FEX's code (`horizon.c:18834-18883`); the loader drops 0x4B/0x4C if unavailable (`loader/source/main.c:446-451`) |
| `svcMapProcessCodeMemory` (0x77) | Code-maps a Normal range at a free address; source becomes borrowed | Autorun's main alias primitive: every Wine view is a heap backing aliased to the guest address (`horizon.c:17451`, `18076-18170`) |
| `svcGetInfo` (0x29), `svcGetSystemInfo` (0x6F) | Memory reporting, section 4 | `GetSystemInfo` is privileged but in our NPDM |

### 2.2 `svcMapPhysicalMemory` in detail [fact]

- Address and size page-aligned, range inside the alias region, and `GetTotalSystemResourceSize() > 0`
  (Mesosphère `source/svc/kern_svc_physical_memory.cpp`; Eden `svc/svc_physical_memory.cpp:33-80`).
- It reserves the size from the process resource limit, allocates pages for the unmapped parts only, fills them with
  the heap fill value, and maps them as `MemoryState::Normal` (`eden k_page_table_base.cpp:5116-5180`). Already mapped
  pages inside the range are skipped, so it is idempotent.
- `svcUnmapPhysicalMemory` requires every mapped page in the range to be Normal **with no attributes** (not borrowed,
  not IPC/device mapped, not uncached), unmaps, and releases the size to the resource limit
  (`eden k_page_table_base.cpp:5411-5547`, checks at `:5444-5450`, release at `:5544`).
- The memory is the same state as heap memory (Normal: `FlagsData | FlagCanCodeMemory`,
  `eden k_memory_block.h:67`), so it can be a source for `svcMapProcessCodeMemory` (which wants a Normal source,
  `eden k_page_table_base.cpp:1187-1190`), for transfer memory and for nvmap.
- Page tables and memory blocks for these mappings come from the process's own system resource (16 MB for Carafe).
  [hypothesis] Heavy 4 KB fragmentation could exhaust it; `InfoType_SystemResourceSizeUsed` (17) shows how close it is.
- Carafe's alias region: `[CARAFE] alias region 0x1e0c200000-0x2e0c200000` (64 GB). Wine is kept out of it
  (`runtime/patches/autorun/0003-horizon-alias-region.patch`), and nothing in Autorun calls `svcMapPhysicalMemory`
  today (the name appears only in NPDM files). The region is free for this use.
- With the `OptimizeMemoryAllocation` NPDM flag (7.0.0+), the kernel remembers which pages this process already had
  and does not clear them again when they are re-allocated to it (`kern_k_memory_manager.cpp`, optimized process
  ids). Carafe's NPDM does not set it. [hypothesis] It would make unmap/remap churn cheaper.

### 2.3 Giving memory back and reusing it

- **Possible [fact]:** physical pages return to the pool by `svcUnmapPhysicalMemory` (any page in the alias region)
  or by shrinking the heap (top only). They can then be allocated again by either call, at any address.
- **Not possible with today's layout [fact]:** the heap cannot release pages in the middle; the only release is the
  2 MB-aligned tail, and that tail must hold no aliased, lent or GPU pages. Autorun aliases heap backings everywhere,
  so in practice the heap never shrinks. Freed memory only returns to Autorun's allocators: newlib malloc grows up
  from the heap start, 64 KB-unit "backing" allocations grow down from the end (`horizon-wine/source/native_heap.c:21-30,
  91-135`, `backing_heap.h:7`).
- **Consequence [fact]:** on Horizon "committed" and "resident" are the same thing. Wine's `MEM_DECOMMIT`/`MEM_RESET`
  cannot hand pages back to the OS unless the backing came from `svcMapPhysicalMemory`.

### 2.4 Demand paging and file-backed mappings: none [fact]

- Every allocating SVC allocates and maps the pages in the same call (`SetHeapSize` `:2190-2200`, `MapPhysicalMemory`
  `:5168-5180` in `eden k_page_table_base.cpp`; Mesosphère's equivalents allocate with `AllocateAndOpen`/
  `AllocateForProcess`, see `docs/research/orchestrator/findings/memory-pages.md`).
- No memory state is file-backed (`eden k_memory_block.h:58-118` lists all states), and no SVC takes a file. The
  kernel has no filesystem; files are served by the `fs` system module over IPC.
- No compression, no page cache in the kernel, no OOM killer: memory a running application holds stays its own until
  it frees it or exits. [hypothesis] This also holds while the application is suspended behind the HOME menu.

### 2.5 User-mode fault handling [fact]

- The kernel delivers EL0 data and instruction aborts (translation, permission, alignment), undefined instructions,
  etc. to the process's exception entry when `KTargetSystem::IsUserExceptionHandlersEnabled()`; otherwise the
  process exits (`source/arch/arm64/kern_exception_handlers.cpp`). The flag comes from the secure monitor's kernel
  configuration (`EnableUserExceptionHandlers`, `kern_k_system_control.cpp`). The saved context includes all
  registers, ESR and FAR.
- Only one thread per process can be inside a user exception at a time; other faulting threads wait in the kernel
  until it calls `svcReturnFromException` (`kern_k_process.cpp`, `EnterUserException`/`LeaveUserException`).
- libnx's entry copies the context to one global dump, calls `svcReturnFromException(0)` at once, then runs
  `__libnx_exception_handler` on one global exception stack (`nx/source/runtime/exception.s`). The kernel lock is
  therefore short, but the stock entry is not safe for two threads at once. Autorun's FEX builds use a per-thread
  exception entry (`horizon-wine/CMakeLists.txt:860-864` requires it for the pager).
- Autorun already resumes after fixing a mapping on the console: lazy commit of reserved ranges on a translation
  fault (`horizon.c:19593-19652`, `horizon_commit_lazy_fault`), transient faults while the kernel holds a page for IPC
  or attribute changes (`horizon.c:16718-16762`), Windows access violations through Wine's dispatcher
  (`horizon.c:16771-16880`).
- So paging in user mode is possible: unmap the alias (and free or unmap its backing), and on the fault restore the
  page and resume. Autorun's experimental pager does exactly that (`dlls/ntdll/unix/horizon_swap.h:26-70`, storage in
  `horizon-wine/source/swap_store.c`/`swap_file.c`; built only with `WINE_NX_SWAP_POC`, default OFF,
  `CMakeLists.txt:860`).
- **What the handler never sees [fact]:**
  - Kernel accesses on our behalf. An IPC buffer or an SVC argument (`svcWaitForAddress`, `svcSignalToAddress`,
    `svcArbitrateLock`) that points to an unmapped page makes the SVC fail instead. Autorun's pager pins all of these
    around the call (`horizon-wine/source/swap_native.c:41-90,170-200`).
  - Device (SMMU) accesses. GPU buffers (nvmap), transfer memory, device-mapped and uncached ranges must stay
    resident; the pager excludes them (`swap_native.c:100-170`).
- Cost of one fault round trip (exception entry + return + restore) is not measured (open question 4).

## 3. GPU memory: the same pool, the same heap

- **[fact]** The Switch GPU (GM20B) is UMA. Mesa's Horizon winsys reports no VRAM and no BAR
  (`mesa-switch:src/nouveau/horizon/nouveau_horizon_device.c:41-44`).
- **[fact]** NVK allocates a buffer's memory with `memalign` from the newlib heap
  (`nouveau_horizon_memory.c:520`), registers it with `nvMapCreate` (nvmap Create + Alloc with the user address,
  `libnx nx/source/nvidia/map.c`), and makes it uncached with `svcSetMemoryAttribute(addr, size, 8, 8)` when the type
  is coherent (`nouveau_horizon_memory.c:461`). GPU virtual addresses are mapped with `MapBufferEx` in 4 KB pages
  (`:825-828`). Buffers are aligned to 64 KB (`NOUVEAU_HORIZON_BIND_ALIGN_B 0x10000`,
  `nouveau_horizon_private.h`).
- **[fact]** Under Autorun, `memalign` of 64 KB or more goes to the "backing" allocator at the top of the heap
  (`native_heap.c:110-135`), so GPU buffers, Wine's guest memory and FEX all compete for one 3168 MB heap. The
  `nvdrv` service also takes 8 MB of transfer memory from that heap (`__nx_nv_transfermem_size = 0x800000`,
  `libnx nx/source/services/nv.c`).
- **[fact]** NVK keeps a BO cache of up to 128 MB (`NOUVEAU_HORIZON_BO_CACHE_DEFAULT_MB 128`, 256 entries, entries
  up to 32 MB, env `NOUVEAU_HORIZON_BO_CACHE_MB`), trimmed on eviction and when an allocation fails once
  (`nouveau_horizon_memory.c:346-347,518-537`). Cached buffers stay registered and uncached, so they are pinned memory.
- **[fact]** Mesa's available-memory query on Switch is replaced by Autorun: heap capacity minus what its allocators
  hand out (`native_heap.c:197-207`; Mesa's own version reads `fake_heap_*` and `mallinfo`,
  `mesa-switch:src/util/os_misc.c`). NVK's budget is 90 % of that (`nvk_physical_device.c:1829-1840`).
- **[fact + inference]** NVK's heap size is `os_get_gpu_heap_size(...)` (`nvk_physical_device.c:1557-1560`). The
  logs match **3/4 of `TotalMemorySize`**: Cat Mail Co `Heap 0: 2.33 GiB` = 0.75 × 3189 MB
  (`autorun_runtime.log:396`); 32-bit NFSU2 `Heap 0: 1.50 GiB` = 0.75 × 2048 MB
  (`docs/research/orchestrator/findings/shared-memory-attributes.md`). The function body was not read.
- **[fact]** GPU buffers cannot be paged by a CPU fault handler (section 2.5): every byte DXVK/NVK has registered is
  resident until NVK frees it.

## 4. Memory reporting

`svcGetInfo` ids (libnx `nx/include/switch/kernel/svc.h`; meanings from Mesosphère `kern_k_process.cpp` and
[switchbrew SVC](https://switchbrew.org/wiki/SVC)) [fact]:

| Id | Name | Meaning |
|---|---|---|
| 2/3 | AliasRegionAddress/Size | Where `svcMapPhysicalMemory` may map |
| 4/5 | HeapRegionAddress/Size | Heap region (8 GB on 39-bit, 2 GB on 32-bit) |
| 6 | TotalMemorySize | `min(heap region size, used + free in our resource limit)` |
| 7 | UsedMemorySize | Heap + mapped physical memory + code + main stack + system resource |
| 16/17 | SystemResourceSizeTotal/Used | Our 16 MB of page-table/memory-block memory and how much is in use |
| 21/22 | Total/UsedNonSystemMemorySize | 6/7 without the system resource |
| 9 | ResourceLimit | Handle; `svcGetResourceLimitCurrentValue/LimitValue/PeakValue` with `LimitableResource_Memory` give the application group's memory |
| 23 | IsApplication | 9.0.0+ |
| 34 | TransferMemoryHint | 19.0.0+ |

- `svcGetSystemInfo(SystemInfoType_TotalPhysicalMemorySize / UsedPhysicalMemorySize, INVALID_HANDLE,
  PhysicalMemorySystemInfo_Application|Applet|System|SystemUnsafe)` gives each pool's total and used size
  (libnx `svc.h`). It is privileged, and our NPDM allows it.
- `UsedMemorySize` counts the whole heap from the start, so it says nothing about real use. Autorun substitutes the
  allocator view (`horizon.c:3899-3925`), and the `[PROGRESS]` line prints `heap_used_mb`/`heap_free_mb` from it.
  Cat Mail Co: used 1003 MB at 5 s, 3030 MB with 52 MB free at 86 s, when `[HMAP] create_backing failed ... errno=12`
  begins (`autorun_runtime.log:2330-2367`).

## 5. SD card as swap storage

- **Bus [fact]:** Horizon drives the SD card in UHS-I SDR104 (104 MB/s ceiling); DDR200 works only under Linux/Android
  ([SoCal tier list 2026](https://guides.socalconsolemodding.com/nintendo-switch-sd-card-benchmark-tier-list-2026/)).
- **Card benchmarks (hekate) [fact]:** good cards read about 96–97 MB/s sequential with 5,200–5,900 random 4 KB IOPS;
  a weak card read 70 MB/s with 2,800 IOPS (same source). Write speed on the Switch was not found in a primary
  source; card makers rate good UHS-I cards at about 90 MB/s sequential write (for example HORI's licensed card,
  "Read 80MB/s, Write 90MB/s"), which is a ceiling, not a measurement through `fs`.
- **Through Horizon's `fs` [fact, Carafe's own measurements]:** a card request costs about 0.83 ms before the
  transfer, then 37–42 MB/s (`sd_read_cache.h:13-21`, quoted in
  `docs/research/orchestrator/findings/sd-read-ahead.md`); loading reads about 300 KB each at about 48 MB/s.
  Cat Mail Co: 758 MB in 3,654 requests taking 20.6 s of card time ≈ 37 MB/s; 1,029 writes took 4.0 s
  (`autorun_runtime.log:2367`).
- **Contention [fact]:** an NSP installed on the SD card reads its RomFS from the same card, and Carafe's overlay
  commits save writes there too (`carafe-overlay.log`). A swap file would share that bandwidth with the game's loading.
- **[hypothesis] Speed in practice:** paging 4 KB at a time would cost ≈0.85 ms per fault (≈5 MB/s). Writing and
  reading in 64 KB–2 MB clusters gets near 40 MB/s: 100 MB of cold data out and back in ≈ 5 s. That fits "keep cold
  data out during a load", not "page the working set".
- **[hypothesis] Wear:** consumer microSD cards are TLC/QLC flash with on-card wear levelling and an endurance of
  roughly a few hundred full-card writes. A 64 GB card at 500 cycles takes about 32 TB; 2 GB of swap writes per hour
  of play would take over 10,000 hours to wear it out. Bursty writes are not the main risk; FAT/exFAT metadata
  updates are, so the swap file should be created at full size once and rewritten in place, never grown or truncated.
- **[hypothesis] Other places:** NAND (eMMC) save or cache storage (NACP `CacheStorageSize`/`TemporaryStorageSize`) is
  another place for a swap file, with unknown speed and its own wear; compressed RAM (macOS-style compressor) needs no
  storage and no kernel support, only CPU time on the A57.

## Implications for Carafe

**Possible**

- Read the real numbers at start and in `[PROGRESS]`: `svcGetSystemInfo` per pool, our resource-limit
  current/peak, `SystemResourceSizeUsed`. All allowed by the current NPDM.
- Give physical memory back to the kernel and take it again: back guest memory (or a part of it) with
  `svcMapPhysicalMemory` in the 64 GB alias region instead of a fixed heap, and unmap pages on decommit. That makes
  Wine's `MEM_DECOMMIT`/`MEM_RESET` real and lets a pager free physical pages without touching the allocator. It needs
  only the system resource Carafe already has; `OptimizeMemoryAllocation` in the NPDM would avoid clearing pages on
  every remap.
- Page in user mode: unmap or alias-remove a page, write it to SD or compress it in RAM, restore it in the fault
  handler. Translation and permission faults are delivered with FAR/ESR and can be resumed; Autorun's
  `WINE_NX_SWAP_POC` is a working design of this.
- On 8 GB modded consoles, use the extra memory: the application pool becomes 6964 MB, the heap region is 8 GB, and the
  loader already sizes the heap from `TotalMemorySize`.

**Impossible**

- More than 3285 MB of application memory on a stock 4 GB console, through NPDM, NACP, resource limits or any SVC.
  Only a different kernel memory arrangement changes that (8 GB mod, or a custom Mesosphère moving applet memory).
- Kernel demand paging, file-backed mappings, kernel swap, page cache or overcommit. Every page is resident while it
  is mapped.
- Paging memory the kernel or the GPU touches directly: IPC buffers during a call, SVC arguments, transfer memory,
  nvmap buffers (all of DXVK/NVK's memory, plus NVK's 128 MB BO cache), uncached or device-mapped ranges.
- Freeing a page in the middle of the `svcSetHeapSize` heap.

**Numbers**

| Item | Value |
|---|---|
| Application pool, 4 GB retail (Erista/Mariko) | 3285 MB |
| Application pool, 6 GB / 8 GB | 4916 MB / 6964 MB |
| Carafe heap on the console (39-bit) | 3168 MB (`TotalMemorySize` 3189 MB) |
| 32-bit titles | `TotalMemorySize` capped at 2048 MB |
| Kept out of the heap by the loader | 96 MB |
| System resource | 16 MB |
| Alias region (free for `svcMapPhysicalMemory`) | 64 GB |
| Lent to services after network init | ~100 MB |
| NVK heap / budget | 3/4 × `TotalMemorySize` (2.33 GiB) / 90 % of free |
| NVK BO cache | up to 128 MB |
| Threads / transfer memories (application) | 96 / 32 |
| SD through `fs` | ~0.83 ms per request + 37–48 MB/s |
| SD raw (good cards, SDR104) | ~97 MB/s read, ~5,900 random 4 KB IOPS |

## Open questions

1. Where do the loader's 96 MB go? The loader can only have taken a 3168 MB heap if `TotalMemorySize` was at least
   3266 MB plus the used part, yet the runtime reads 3189 MB at `[INIT]`. Something charges ~96 MB to our resource
   limit before the runtime starts, outside "used memory" as the kernel counts it. Log `TotalMemorySize`,
   `UsedMemorySize`, the resource limit's current value and `svcGetSystemInfo(Application)` in the loader and again
   at runtime start.
2. Is the 96 MB `HEAP_RESERVE` still needed, and for what? The source has no comment on it
   (`loader/source/main.c:17`). If it is consumed by a system service on our behalf, that service is a candidate for
   accounting.
3. Which pool backs NACP `JitConfiguration.MemorySize`, and does any service (nvservices, `jit`, `am`) charge the
   application resource limit?
4. Cost of one user-mode fault round trip on the A57 (exception entry, `svcReturnFromException`, restore), and of one
   `svcMapPhysicalMemory`/`svcUnmapPhysicalMemory` pair for 64 KB and 2 MB. These decide the granularity of any pager.
5. How fast does `SystemResourceSizeUsed` grow with many 4 KB physical mappings? Is 16 MB enough for a pager working
   at 64 KB units over 3 GB?
6. Real SD write throughput and latency through `fs` for 64 KB–2 MB writes to a preallocated file, on a typical card
   and on a weak one, while the game also reads its RomFS.
7. Does `IsUserExceptionHandlersEnabled` hold on every retail console with Atmosphère (Autorun depends on it already,
   so probably yes), and does it hold under the stock kernel without Atmosphère patches? (Carafe always runs on
   Atmosphère, so this is only for completeness.)
