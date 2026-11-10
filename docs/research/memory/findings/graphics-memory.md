# Graphics memory on the Switch: what the stack costs, where it is duplicated, what can be saved

Research snapshot: 2026-11-05. For subsequent runtime changes and validation, see the
[report](../REPORT.md#updates-after-the-study).

## Answer

The game, Wine, FEX, DXVK and Mesa all draw from one pool: the process's `TotalMemorySize` (3189 MB in the Cat Mail
Co run of 2026-11-05, 2048 MB in the NFSU2 runs of 2026-11-04). NVK reports three quarters of it as one
device-local heap (2.33 GiB or 1.50 GiB) with two memory types, both CPU-visible; DXVK therefore treats the console as
a unified-memory device: it never evicts, never enforces a budget, and uses the budget only to decide when to free
empty chunks. What games are told is larger than what exists: a 64-bit D3D11 game sees 2391 MiB of dedicated video
memory plus 2391 MiB of shared memory on a 3189 MB machine; a D3D9 game sees `GetAvailableTextureMem` of 2383 MiB
(1528 MiB on the 2048 MB runs).

The copies that cost the most are the ones the D3D9 and D3D11 designs make for system-memory semantics, which on a
desktop live in RAM next to VRAM and here live in the same pool:

1. **D3D9 managed textures keep a full CPU copy for their whole life** (32-bit: an anonymous file mapping; 64-bit: a
   host-visible Vulkan buffer). Textures are stored twice. This is likely the single largest duplicate for D3D8/D3D9
   games (estimate 100–400 MB).
2. **D3D9 static and managed vertex/index buffers keep a persistent staging buffer** next to the device-local one:
   buffer memory twice (estimate 20–100 MB).
3. **Memory the drivers keep after it is freed**: DXVK keeps empty chunks (up to one 256 MB device chunk plus four
   64 MB mapped chunks per memory type on 64-bit) for up to 20 s unless over budget, and mesa-switch keeps up to 128 MB
   of freed backings in its BO cache, released only when a Vulkan allocation fails. Both peak exactly at level loads,
   where Cat Mail Co ran out of memory.
4. **D3D11 load-time staging**: up to 3 × 48 MB of upload staging in flight on 64-bit DXVK during resource creation.

Fixed costs of the screen path are small by comparison: about 11 MB for the 1280×720×3 swapchain, about 11 MB for the
floating keyboard's per-image upload buffers once it has been shown, a few 3.7 MB layers in the OpenGL compositor, and
an nvc0 OpenGL context that stays alive while a Vulkan game runs (size not measured). Shader caches are negligible in
memory (the DXVK cache file of NFSU2 is 156 KB).

Two of Carafe's own option hints are wrong for this device: `dxvk.maxMemoryBudget` does not cap what DXVK allocates on
UMA (it only frees empty chunks sooner and makes defragmentation start), and `d3d9.textureMemory` is not an upload
buffer and does not save physical memory (it bounds how many managed-texture copies are mapped into the 32-bit address
space at once).

Nothing here measures how many MB each part holds in a real game: no log line reports DXVK's allocator statistics,
NVK's heap usage, the BO cache, or D3D9's memory files. The first step is that line (see the end).

## Trees and sources

- DXVK 3.1.1, `b1a1c99ab52b687cf950d62c88bc2fa316b41663` (`runtime/dxvk/build-dxvk.sh:7`), source in
  `runtime/build/dxvk-src`; paths below like `src/dxvk/dxvk_memory.cpp` are relative to
  it. Carafe's DXVK patches in `runtime/patches/dxvk/0001–0022`.
- Console Mesa: `danfromtico/mesa-switch@d4a00ea0ab` (Mesa 26.2.3, pinned in the build manifest,
  `runtime/build/wine-switch/stage/switch/wine/build-manifest.json`, `"mesa_switch"`). Not on disk; prebuilt into the
  `switch-dev` image and linked by `external/autorun/horizon-wine/CMakeLists.txt:903-968`. Files below marked
  *mesa-switch* were read from GitHub at that commit through a summarising fetch: quoted code is as returned, line
  numbers are approximate. The lab's Mesa (`carafe-lab/external/mesa`, upstream 25.2.8, `f97d7dc`) has no Horizon
  winsys and is used only as a proxy for NVK's generic code; it is marked *Mesa 25.2.8*.
- Autorun at the submodule commit in `carafe/external/autorun`.
- Console logs: `console-logs/catmail-2026-11-05/logs/CatMailCo.log` (Unity, 64-bit, D3D11) and
  `console-logs/nfsu2-2026-11-04/logs/autorun_runtime.log` (32-bit, D3D9).
- Earlier findings this builds on: `docs/research/orchestrator/findings/shared-memory-attributes.md` (memory types,
  NvMap, uncached mappings) and `docs/research/dxvk-sarek/REPORT.md`.

## 1. NVK on Horizon: heaps, types, budget, allocation

### Facts

- One heap, device-local, sized from the process's memory: Cat Mail Co logs `memory=3189 MB` (`CatMailCo.log:23`) and
  `Heap 0: 2.33 GiB (DEVICE_LOCAL)` (`:353`); NFSU2 logs `memory=2048 MB` and `Heap 0: 1.50 GiB`. Both are exactly 75 %
  of the total (3189 × 0.75 = 2391 MiB = 2.335 GiB; 2048 × 0.75 = 1536 MiB). In mesa-switch the size comes from
  `os_get_gpu_heap_size(instance->drirc.misc.heap_memory_percent, …)` (*mesa-switch* `nvk_physical_device.c`, the
  helper's body not read); Mesa 25.2.8 has the same rule spelled out: "Use 3/4 of total size to avoid swapping"
  (*Mesa 25.2.8* `src/nouveau/vulkan/nvk_physical_device.c:1295-1304`).
- Total and available memory on Horizon: `svcGetInfo(InfoType_TotalMemorySize)`, and available = total −
  `InfoType_UsedMemorySize` (*mesa-switch* `src/util/os_misc.c`, about lines 370 and 415-421). So "available" counts
  everything the process uses: guest memory, FEX, Wine, and Vulkan memory itself.
- The winsys reports no VRAM and no BAR (*mesa-switch* `src/nouveau/horizon/nouveau_horizon_device.c`,
  `vram_size_B = 0`, `bar_size_B = 0`); on a SoC NVK flags system memory device-local and makes a cached and an
  uncached type (`shared-memory-attributes.md`, lines 41-49). Logged: `Type 0: DEVICE_LOCAL | HOST_VISIBLE |
  HOST_CACHED`, `Type 1: DEVICE_LOCAL | HOST_VISIBLE | HOST_COHERENT` (`CatMailCo.log:355-356`). Both types are the
  same physical memory; only the CPU cache attribute differs.
- `VK_EXT_memory_budget` is supported (`extMemoryBudget : 1`, `CatMailCo.log:527-528`); `VK_EXT_memory_priority`
  and `VK_EXT_pageable_device_local_memory` are not (`:529-536`).
- The budget follows free memory. Logged budgets: 1.37 GiB for the first D3D11 device at 5 s, when Autorun reported
  `heap_free_mb=2079` (`CatMailCo.log:339, 354`), 1.15 GiB for the second device (`:684`); NFSU2 909–965 MiB. These
  fit budget = 0.9 × min(0.75 × available + used, heap): 0.9 × 0.75 × 2079 MB = 1403 MiB = 1.37 GiB. Mesa 25.2.8
  has exactly that (`nvk_physical_device.c:1307-1317`, `:1580-1606`, "Set the budget at 90% of available");
  mesa-switch calls `vk_physical_device_heap_budget(available, percent, heap->size, used)` (body not read).
- NVK does not refuse an allocation for exceeding the heap size: it only counts `heap->used`
  (*Mesa 25.2.8* `nvk_device_memory.c:270, 297`). The heap size is advisory; the real limit is the process's memory.
- Allocation: backings are `memalign` memory from the native heap, made GPU-visible with `nvMapCreate`
  (`shared-memory-attributes.md:56-60`). Non-imported allocations are aligned to the bind alignment, 64 KiB
  (`NOUVEAU_HORIZON_BIND_ALIGN_B 0x10000u`, *mesa-switch* `src/nouveau/horizon/nouveau_horizon_private.h:36`;
  `align_B = MAX2(create_info->align_B, bind_align_B)`, *mesa-switch* `nouveau_horizon_memory.c` about 458-464).
- **BO cache**: freed backings go into a cache of up to 128 MB, at most 256 entries, entries of at most 32 MB, in ten
  size buckets, LRU (`NOUVEAU_HORIZON_BO_CACHE_DEFAULT_MB 128`, `…_MAX_ENTRIES 256u`, `…_MAX_ENTRY_B (32ull << 20)`,
  `nouveau_horizon_private.h:47-50`; LRU put/evict in `nouveau_horizon_memory.c` about 337-375). The cap is read from
  the environment variable `NOUVEAU_HORIZON_BO_CACHE_MB` when the device is made (`debug_get_num_option(…)`, about
  372-375; 0 disables it). The cache is trimmed when an allocation fails and at device destruction (about 379-381,
  394). It exists to avoid "kernel cache-attribute transitions" on reuse (`shared-memory-attributes.md:60`).
- Image sizes are rounded to at least 4 KB, and to 64 KB for compressible kinds (*Mesa 25.2.8*
  `src/nouveau/nil/image.rs:426-456`); on the Switch every allocation is pitch-kind
  (`shared-memory-attributes.md:53-55`), so the 4 KB rule applies.
- Carafe already sets one mesa-switch variable before Vulkan starts: `setenv("NVK_SWITCH_CPU_WRITE_MEM_UNCACHED",
  "0", 1)` in a constructor (`runtime/overlay/carafe_settings.c:537-545`). Any other Mesa variable can be set there.

### What it means

- The BO cache is memory that neither the game nor DXVK can get back: it counts as used for the kernel, it is not in
  DXVK's accounting, and nothing trims it when the *guest* runs out (Cat Mail Co's `VirtualAlloc` failure did not go
  through Vulkan). DXVK's first chunks (4, 8, 16, 32 MB) are exactly the sizes it accepts.
- The heap size steers what DXGI and D3D9 report (section 4). If mesa-switch's `heap_memory_percent` can be set from
  the environment, one `setenv` changes what every D3D game is told. *Hypothesis*: Mesa's driconf reads an
  environment variable of an option's name (`getenv(cache->info[opt].name)`, *Mesa 25.2.8*
  `src/util/xmlconfig.c:876`); not verified for mesa-switch or for this option.

## 2. DXVK's allocator on a unified-memory device

### Facts (DXVK 3.1.1)

- UMA detection: every heap device-local (`src/dxvk/dxvk_adapter.cpp:375-383`). On the Switch this is true.
- Budget enforcement and eviction are off on UMA: `heap.enforceBudget = !isUnifiedMemoryArchitecture() && DISCRETE`
  (`src/dxvk/dxvk_memory.cpp:754-755`); eviction is only enabled when an enforced budget is exceeded (`:1352-1356`) and
  is skipped otherwise ("Doing this on integrated graphics would be harmful", `:2641-2643`).
- The budget DXVK uses is NVK's budget minus driver-internal usage minus 1 % of the heap (clamped to 4–256 MB), capped
  by `dxvk.maxMemoryBudget` if set (`:2432-2466`). It is re-read every 500 ms (`:2731-2760`). On UMA it is used for
  two things only: freeing empty chunks when `allocated > budget` (`:1773-1776`), and starting defragmentation under
  "memory pressure" (`:2537-2560`).
- Chunk sizes: pages allocator chunks 4–256 MB (`src/dxvk/dxvk_memory.h:1128-1129`). Device (not mapped) pools: up to
  256 MB, halved until seven fit in the heap (`dxvk_memory.cpp:1978-1996`, `MinAllocationsPerHeap = 7`,
  `dxvk_memory.h:1136`): 256 MB on a 2.33 GiB heap, 128 MB on a 1.50 GiB heap. Mapped pools: max/4 = 64 MB on 64-bit,
  max/16 = 16 MB on 32-bit (`:1984-1985`). Pools start at 4 MB and double when the next size is at most half of what
  the type has allocated (`:1522-1524`).
- Which type: a device-local-only request matches both types and takes the first, type 0 (CPU-cached); a mapped
  request needs `HOST_COHERENT`, which only type 1 has (`:2154-2214`, and `shared-memory-attributes.md:93-99`). So
  images and device-local buffers live in type 0's device pool, staging and mapped buffers in type 1's mapped pool.
- **Empty chunks kept**: per pool, empty chunks up to one maximum chunk (device pool) or four maximum chunks (mapped
  pool) are kept; each is freed after 20 s unused, one per pass, or at once when over budget (`:1743-1800`). On 64-bit:
  up to 256 MB (type 0 device) + 4 × 64 MB (type 1 mapped) = 512 MB of empty chunks; on 32-bit with a 1.5 GiB heap:
  128 + 4 × 16 = 192 MB.
- Very large resources get dedicated allocations instead of chunks (`:885-899`).
- Defragmentation (`dxvk.enableMemoryDefrag`, Auto = on for NVK, `:2783-2798`) moves resources out of the sparsest
  device-pool chunk when more than an eighth of pages are wasted or when allocated exceeds the budget
  (`:2530-2620`). Mapped pools are never defragmented ("we rely on pointer stability there", `:2766-2768`).
- `dxvk.maxChunkSize` does not exist in 3.1.1 (no `getOption` for it in `src/`); chunk limits are compile-time.

### What it means

- On the Switch DXVK cannot overcommit "VRAM" into "system memory" because both are the same; when the process runs
  out, allocations simply fail. DXVK's only reaction to pressure is to free empty chunks, 500 ms late.
- `dxvk.maxMemoryBudget` (Carafe's "DXVK memory budget", suggested on memory failures, `settings_catalog.rs:1135-1136`)
  therefore does not limit textures or buffers. A low value makes DXVK free every empty chunk at once (useful at
  level changes) and keeps defragmentation running permanently, since allocated stays above the budget. The hint
  "How much memory DXVK takes for textures and buffers … textures may load in late"
  (`app/ui/src/shared/i18n/locales/en.json:46, 90`) describes eviction, which does not happen here.

## 3. Copies DXVK keeps

### D3D9 managed textures — the largest duplicate

- Map mode: on 32-bit DXVK (`DXVK_USE_UNMAPPABLE_MEMORY` is defined for `_WIN32 && !_WIN64`,
  `src/util/util_unmap.h:12-14`), any texture not in `D3DPOOL_DEFAULT` gets `UNMAPPABLE` when `d3d9.textureMemory != 0`
  (`src/d3d9/d3d9_common_texture.cpp:617-629`): its full contents live in a `MemoryFileRegion` (`:97-98`), allocated
  from 16 MB anonymous file mappings, `CreateFileMappingA(INVALID_HANDLE_VALUE, …, PAGE_READWRITE | SEC_COMMIT, …)`
  (`src/util/util_unmap.cpp:72-79`, `DefaultChunkSize = 16ull << 20u`, `util_unmap.h:107`).
- On 64-bit DXVK, managed and systemmem textures are `BACKED`: a persistent buffer requested as host-visible,
  coherent, cached (`d3d9_common_texture.cpp:335-352`), which lands in type 1, uncached.
- The copy is never freed for managed textures: after an unlock, DXVK tosses the buffer only for non-dynamic,
  *non-managed* textures without format conversion ("Toss our staging buffer if we're not dynamic and we aren't managed
  (for sysmem copy.)", `src/d3d9/d3d9_device.cpp:5550-5561`). `EvictManagedResources` does nothing (`:266-268`).
- So every managed texture exists twice: CPU copy and GPU image. Desktop D3D9 does the same (the runtime keeps a
  system-memory copy of the managed pool), so this is faithful, but on a desktop the copy sits in RAM next to VRAM;
  here both come from the 3.2 GB.
- `d3d9.textureMemory` (default 100 MB, `src/d3d9/d3d9_options.cpp:73`) only bounds how much of those memory files is
  *mapped*: past it, `UnmapTextures` unmaps least recently used textures down to 75 % (`d3d9_device.cpp:9393-9413`).
  The section's pages stay. Under Autorun, a view aliases the section's pages (`horizon_memfile_alias` /
  `horizon_memfile_map`, `external/autorun/dlls/ntdll/unix/horizon.c:19191-19244`; section views tracked at
  `:14460-14516`), so there is one CPU copy, not one per view. Whether `SEC_COMMIT` commits the whole 16 MB section at
  creation was not checked (outside this study's scope).
- `d3d9.textureMemory = 0` switches 32-bit DXVK to the 64-bit scheme (a host-visible Vulkan buffer per managed
  texture, in 16 MB mapped chunks): the same copy, now in uncached Vulkan memory and in the 32-bit address space.
- Carafe's hint calls it the "Texture upload buffer (DX9)" and says "Lower saves memory, but textures load with
  hitches" (`en.json:94`); Autorun's says "D3D9 texture upload buffer size in MiB. Lower values save memory"
  (`horizon-wine/source/dxvk_options.c:263-265`). Both are wrong: lower values save address space and cost
  map/unmap calls through the Horizon server; physical memory is unchanged.

### D3D9 buffers

- Every buffer except `D3DPOOL_DEFAULT` + `D3DUSAGE_DYNAMIC` uses `MAP_MODE_BUFFER` (`src/d3d9/d3d9_common_buffer.cpp:103-113`):
  a device-local buffer plus a staging buffer the game locks (`:151-173`; `GetStagingBuffer`, `d3d9_common_buffer.h:240`).
  The staging buffer is never released. Static `DEFAULT` and `MANAGED` vertex and index buffers therefore exist twice.
- Dynamic `DEFAULT` buffers are mapped directly (one copy), in type 1.
- Carafe's patch `0012` adds, with `d3d9.managedBufferCopies` (on by default since `0015`,
  `runtime/patches/dxvk/0015-…:16-20`), a third, CPU-side copy for managed buffers of at most 4 MB that are locked a
  second time (`CopiedBufferMax = 4u << 20`, `0012-…:157-158`). Small for most games; it grows with the number of
  managed buffers a game re-locks.
- Patch `0010` makes the front-end queue 4 MB instead of 1 MB (`0010-…:1`); with the effect thread there are two
  front ends. A few MB.

### D3D11

- Textures without CPU access have no copy (`MAP_MODE_NONE`, `src/d3d11/d3d11_texture.cpp:561-563`). `DEFAULT`
  textures with CPU access keep a persistent staging buffer (`:565-568`); `STAGING` textures are buffers only, no image
  (`:570-574`); dynamic textures whose format needs packing use a temporary buffer (`:576-587`).
- Initial data goes through staging memory: up to 48 MB per submission on 64-bit (12 MB on 32-bit) and three times that
  in flight (`src/d3d11/d3d11_initializer.h:24-35`). On 64-bit that is up to 144 MB of staging during loads, in the
  type 1 mapped pool, which then keeps up to 256 MB of empty chunks for 20 s (section 2).
- DXVK 3.1.1 does not use `VK_EXT_host_image_copy` (no use in `src/`), which on a UMA device would write initial data
  straight into images without staging.
- Unity made two D3D11 devices in Cat Mail Co (`Using feature level D3D_FEATURE_LEVEL_11_1`, `CatMailCo.log:638`, and
  `…11_0`, `:967`), each with its own 8 MB descriptor heap (`:640, :969`). Whether the first stays alive is not known.

### 32-bit address-space features

- Pipeline lifetime tracking is on for 32-bit DXVK on any driver but RADV (`src/dxvk/dxvk_device.cpp:133-151`):
  libraries built ahead are destroyed and rebuilt at first use. It saves address space (which under WoW64 the driver
  does not use) and some physical memory for pipeline libraries; `REPORT.md` of the Sarek work measured its time cost.
- `env::is32BitHostPlatform()` also shrinks mapped chunks (16 MB), the HVV chunk limit (`dxvk_memory.cpp:875`) and the
  D3D11 staging limit; all of these cut peak memory slightly, as a side effect.

## 4. What games are told

### Facts

- DXGI: `deviceMemory` = the largest device-local heap, `systemMemory` = the sum of the others
  (`src/dxvk/dxvk_adapter.cpp:77-85`). With one heap, systemMemory is 0 and DXGI "over-reports" shared memory as equal
  to device memory (`src/dxgi/dxgi_adapter.cpp:459-462`). Powers of two above 4 GB get 32 MB taken off (`:473-479`),
  `dxgi.maxDeviceMemory`/`dxgi.maxSharedMemory` cap both (`:484-490`, options in `dxgi_options.cpp:169-170`), and
  32-bit processes clamp to 3 GB (`:493-498`). Result on the console: **DedicatedVideoMemory 2391 MiB and
  SharedSystemMemory 2391 MiB** for a 64-bit game with 3189 MB in total; 1536 MiB each with 2048 MB.
- The reported GPU is an AMD one: `DXGI: Hiding actual GPU, reporting: vendor ID: 0x1002, device ID: 0x73df`
  (`CatMailCo.log:318-320`; fallback `0x73df` at `dxgi_adapter.cpp:424`). 0x73DF is a Radeon RX 6700 XT-class device.
- `QueryVideoMemoryInfo` returns DXVK's budget and its own allocated amount (`dxgi_adapter.cpp:290-316`), so engines
  that ask for the budget see one that shrinks as the process fills.
- D3D9 `GetAvailableTextureMem` starts at min(device + system heaps, `d3d9.maxAvailableMemory` = 4096 MiB) − 8 MB
  (`src/d3d9/d3d9_device.cpp:6283-6304`; default at `d3d9_options.cpp:53`) and goes down only for non-managed
  textures (`d3d9_device.cpp:250-263`, `d3d9_device.h:1112-1119`): **2383 MiB** on 3189 MB, **1528 MiB** on 2048 MB.
  Managed textures are not subtracted.

### Hypotheses (engine behaviour; not read in engine source here)

- Unity: `SystemInfo.graphicsMemorySize` comes from DXGI's dedicated memory. Unity's built-in renderers do not pick
  texture quality from it; its texture-streaming budget is a quality setting the game ships. Only games that read the
  value themselves react. Lowering `dxgi.maxDeviceMemory` is therefore per-game, with an uncertain effect.
- Unreal Engine 4/5: the texture streaming pool can be sized from dedicated video memory
  (`r.Streaming.PoolSizeVRAMPercentage`, used when no fixed `r.Streaming.PoolSize` is configured) and engines that use
  `QueryVideoMemoryInfo` adapt to the budget. A 2.3 GB "VRAM" plus an RX 6700 XT identity invites high presets and a
  large pool. Worth reading per game (its `DefaultEngine.ini`/device profiles) before changing anything.
- UE3 (Dishonored, BioShock-era): the pool size comes from the game's ini, not from VRAM.
- D3D9 games that choose detail from `GetAvailableTextureMem` see 1.5–2.3 GB and pick their highest level; a desktop
  card of 2004–2008 had 128–512 MB.

## 5. Shader and pipeline caches

- DXVK 3.1.1's state cache is gone (Autorun lists `dxvk.enableStateCache` only up to version 2.7,
  `dxvk_options.c:233-235`). Its new shader IR cache keeps a look-up table in memory (`std::unordered_map … m_lut`,
  `src/dxvk/dxvk_shader_cache.h:128`) and reads binaries from the file on demand (`dxvk_shader_cache.cpp:279-300`). The
  NFSU2 cache is 156 KB on disk (`nfsu2-2026-11-04/drive_c/users/steamuser/AppData/Local/dxvk/a69b094234a3ca32.dxvk.bin`).
  Negligible.
- Mesa's disk cache runs (`[NATIVE] Mesa queue disk$: 4 threads, 32 jobs, flags 0x7`, `CatMailCo.log:315`): four
  native threads and their stacks, plus queued writes. *Hypothesis*: under 5 MB in memory; not measured.
- The memory that matters is compiled pipelines and shader code inside NVK (its shader heap in GPU memory) and DXVK's
  shader objects: unmeasured; it grows with the number of pipelines (thousands in an open-world game). *Hypothesis*:
  tens of MB.

## 6. Screen, compositor and overlay buffers

| Buffer | Size | Lifetime | Source |
|---|---|---|---|
| Text-console framebuffer, 1280×720 RGBA ×3 (+ linear copy) | ~11–15 MB | until a GL or Vulkan surface takes the screen; closed at `runtime.c:678` | `external/autorun/horizon-wine/source/runtime.c:530-531, 568-575` |
| OpenGL compositor (nvc0 GL 4.3 context) | context: not measured; EGL window buffers 2–3 × 3.7 MB | context kept for the whole run; only the surface is destroyed when a program takes the screen | `compositor.c:355-367`, `compositor_egl.c:93-110`; `[NXCOMP] presenting … Mesa 26.2.3`, `CatMailCo.log:33` |
| Compositor layer per window | CPU pixels w×h×4 + GL texture w×h×4 (7.4 MB at 1280×720) | while the window lives | `compositor.c:94-103`, `compositor_gl.c:157` |
| DXVK swapchain | 1280×720 × 3 images = 11 MB | game's | `CatMailCo.log:1061-1066` |
| Scaled-mode host images (game mode ≠ 1280×720) | 1280×720 × images, plus the game-size images | game's | `external/autorun/dlls/win32u/vulkan.c:3008-3022` |
| Floating keyboard / menu copy into the swapchain | 1280×720×4 per swapchain image = 11 MB | allocated on first show, kept for the swapchain's life | `vulkan.c:3841-3912` (size at `buffer_info.size = width * height * 4`); `[NXVK] floating keyboard 1280x720 at 0,0 on 3 images`, `CatMailCo.log:1069` |
| Loading screen picture, splash GIF | 3.7 MB picture; GIF 256×80×32 frames = 0.65 MB | GIF kept (`g_movie` never freed) | `runtime/overlay/carafe_loading.c:339, 821, 1421`; overlay log `StartupMovie.gif 256x80, 32 frames` |

Total fixed cost of the screen path while a Vulkan game runs: roughly 25–45 MB, of which the nvc0 context is the
unknown part.

## 7. Carafe's options today

The catalog's "dxvk-memory" page offers `dxvk.maxMemoryBudget`, `dxgi.maxDeviceMemory`, `dxgi.maxSharedMemory`
(Auto, 128–4096 MiB), `d3d9.maxAvailableMemory` (128–4096 MiB), `d3d9.textureMemory` (16–256 MiB),
`dxvk.enableMemoryDefrag`, `dxvk.trackPipelineLifetime` (`app/crates/carafe-core/src/settings_catalog.rs:738-746,
776-792`), and `d3d11.disableMsaa`, LOD bias rows elsewhere (`:836-840`). On a memory failure the app suggests
`d3d9-maxAvailableMemory` and `dxvk-maxMemoryBudget` (`:1130-1137`). Autorun carries them to the console's settings
list (`dxvk_options.c:230-232, 254-266, 296-298`). Nothing sets a Mesa memory variable and nothing changes DXVK's
copy policy.

## 8. Savings

Estimates for a typical 32-bit D3D9 game (NFSU2/Carbon class: ~200 MB of managed textures, ~50 MB of static
geometry) and a typical 64-bit D3D11 Unity game (Cat Mail Co class). All MB figures are estimates unless a row says
otherwise; the instrumentation in the next section turns them into measurements.

| # | Idea | D3D9 (32-bit) | D3D11 Unity (64-bit) | Risk | Kind |
|---|---|---|---|---|---|
| 1 | **Drop a managed texture's CPU copy after its upload** (extend the toss at `d3d9_device.cpp:5550-5561` to managed textures not locked for N frames; on a later lock, recreate the buffer and read the image back, as `DEFAULT`-pool textures already do) | 100–400 MB, the size of the managed set | 0 (no managed pool in D3D11) | Medium–high: a later read-back is a GPU sync plus uncached reads; textures relocked often (streaming, animated) stall; format-converted textures must keep the copy (as today) | DXVK patch, Experimental row, per game; a default only after many games pass |
| 2 | **Static `DEFAULT` buffers mapped directly** on UMA (one host-visible device-local buffer, no staging) | 20–100 MB | small (D3D11 immutable buffers already have no copy) | Medium: locks without `DISCARD`/`NOOVERWRITE` wait for the GPU; reads are uncached (`shared-memory-attributes.md`) | DXVK patch, Experimental, per game |
| 3 | **Cap mesa-switch's BO cache**: `setenv("NOUVEAU_HORIZON_BO_CACHE_MB", "32", 1)` next to the existing `NVK_SWITCH_…` line (`carafe_settings.c:540`) | up to 96 MB at peaks | up to 96 MB at peaks (loads) | Low: more first-time uncached allocations (a clean + `svcSetMemoryAttribute` each), seen as rare hitches when DXVK grows a pool | Default change after one console A/B; or a row |
| 4 | **Keep fewer empty DXVK chunks on UMA**: mapped pool keeps one chunk instead of four; free empty chunks after 2 s instead of 20 s | 0–110 MB (192 MB bound) | 0–380 MB (512 MB bound), transient at loads | Low: more allocate/free churn at loads; combined with #3 each new chunk is a fresh NvMap | DXVK patch; default candidate. Not `dxvk.maxMemoryBudget`, which also starts permanent defragmentation |
| 5 | **Smaller D3D11 load staging on UMA**: use the 32-bit limits (12 MB per submission, 36 MB in flight) on 64-bit | 0 | up to ~108 MB less peak during loads | Low: loads slightly slower (more submissions) | DXVK patch, default candidate |
| 6 | **Report less video memory**: `dxgi.maxDeviceMemory`/`maxSharedMemory` ≈ 1024–1536, `d3d9.maxAvailableMemory` ≈ 512–1024 | 0–200 MB, only in games that size by it | 0–500 MB, only in engines that size pools by it (UE4/5) | Medium: games that require a minimum VRAM refuse to start or pick a low preset; gains are game-specific | Per game; the rows exist. As a default only after a survey of the test games |
| 7 | **Shrink the reported heap** with `heap_memory_percent` (if mesa-switch accepts it from the environment): changes #6 for every game at once | as #6 | as #6 | Medium, plus unverified mechanism; NVK does not enforce the heap, so allocations are not capped | Default candidate only after verification |
| 8 | **Floating keyboard copy buffers sized to the keyboard and freed when it hides** (`vulkan.c:3841-3912`) | 7–11 MB after first show | 7–11 MB | Low | Autorun patch, default |
| 9 | **Free the nvc0 compositor context while a Vulkan swapchain owns the screen** (`egl_quit` instead of `egl_detach`, re-init on return) | 5–20 MB (*hypothesis*) | same | Medium: the context must come back for the menu over GDI windows, dialogs, the error screen | Autorun patch, after measuring the context |
| 10 | **Release the first D3D11 device's memory** if Unity keeps its probe device | 0 | 10–40 MB (*hypothesis*, if it lives) | Unknown | Investigate first |
| 11 | **No MSAA** (`d3d11.disableMsaa`, `d3d9.forceSwapchainMSAA = 0`) | 15–30 MB at 1280×720 4×MSAA | same | Visual (aliasing) | Per game; rows exist |
| 12 | **Fix the two hints** (`en.json:46, 90, 94`; Autorun `dxvk_options.c:230-232, 263-265`) and stop suggesting `dxvk-maxMemoryBudget` on memory failures unless #4 is not done | — | — | None | Text change |

Largest expected effects: #1 for D3D8/D3D9 games, #3 + #4 + #5 for load-time peaks in any game (Cat Mail Co's failure
was a 4 MB `VirtualAlloc` during a load, with `heap_used_mb=3030 heap_free_mb=52`, `CatMailCo.log:2324`), #6/#7 for
engines that size by VRAM.

## 9. Needs measuring

| Measurement | How | Decides |
|---|---|---|
| `[GPU MEM]` line every 10 s: per heap NVK `heapUsage`/`heapBudget`; DXVK `memoryAllocated`/`memoryUsed` per type and pool, empty chunks; D3D9 memory-file stats (`m_memoryAllocator.getStats()`: allocated, used, mapped); NVK BO cache bytes | DXVK patch next to `0005`'s counters (DXVK has the numbers: `getMemoryStats`, `dxvk_memory.cpp:2226`); BO cache size needs a mesa-switch symbol or is inferred from `heapUsage` vs DXVK's allocated | Every row above; how much of Cat Mail Co's 3030 MB is graphics |
| Managed texture bytes and how often a managed texture is relocked after its first upload, per game | counters in the D3D9 lock path | #1's value and its N |
| Static-buffer bytes, and locks of static buffers without `DISCARD`/`NOOVERWRITE` | same | #2 |
| nvc0 context size: `UsedMemorySize` before and after `egl_quit` at close | Autorun log at the closing step | #9 |
| What Unity/UE games read: Unity's `Player.log` `VRAM:` line, UE logs of the streaming pool size | pull `AppData/LocalLow/<company>/<game>/Player.log` with the run's logs | #6 |
| Whether `heap_memory_percent` is read from the environment in mesa-switch | read `src/util/os_misc.c`/driconf code at `d4a00ea0ab`, then one run with the variable set and the logged `Heap 0` | #7 |
