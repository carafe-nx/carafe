# Memory shared with the GPU: attributes and CPU cost (C7)

## Answer

On the console every buffer that DXVK maps for the CPU is CPU-uncached. NVK on the Switch exposes two memory types,
both on the one 1.5 GB heap: type 0 is CPU-cached but not coherent, type 1 is coherent because it is CPU-uncached.
DXVK always asks for coherent memory, and when it asks for "cached and coherent" (for buffers the CPU may read) no
type matches, so it falls back to type 1. Type 1 is made by `svcSetMemoryAttribute(..., Uncached)`, which the kernel
maps as ARM "Normal Non-cacheable" (MAIR attribute `0x44`), the ARM equivalent of write-combined memory. NVK's own
command buffers and upload streams are uncached too (a Switch-only default). So every CPU path into the GPU is a
stream of stores to non-cacheable memory: DrawPrimitiveUP vertex copies (up to 260 a frame in the race), shader
constant uploads (DXVK copies each dirty constant set per draw, up to 2200 sets a frame), buffer locks (up to 250 a
frame), NVK's push buffers on `dxvk-cs`. Sequential full-line stores to such memory are expected to merge in the store
buffer and cost about as much as cached stores; reads cost a full DRAM access each (at least 377 cycles, about 200 ns,
when dependent) with no cache and no prefetch, which makes a bulk read roughly 5–40× slower than from cached memory
(an estimate; nothing here was timed on the A57). No per-frame path inside DXVK or NVK reads this memory: query
results are the one CPU read path, and NVK keeps them in cached memory with an explicit invalidate. The reads that do
happen come from the game: a lock of a buffer without `D3DUSAGE_WRITEONLY` that the game reads, `GetRenderTargetData`,
locks of render targets and `D3DPOOL_DEFAULT` textures. Whether NFSU2 does any of these per frame is not known; the
main thread's 0.2–0.3 % in `ucrtbase.dll` (where DXVK's `memcpy` lands) bounds the copies DXVK makes on it.

## Evidence

Trees, and which one the console runs:

- **Console's Mesa**: `danfromtico/mesa-switch` at `d4a00ea0ab3f59afb967cc5d779e4263d237bd77` (2026-09-28, `VERSION`
  26.2.3). The console log prints `Mesa 26.2.3 (git-d4a00ea0ab)` and `NVK 26.2.3` (`logs/SPEED2.log:34, 364, 392`). It
  is prebuilt into the `switch-dev:2026.10.01` image and linked as `libvulkan.a` by Autorun
  (`external/autorun@636913ca:horizon-wine/CMakeLists.txt:903-937`); the pin is in
  [../../loading-screen/REPORT.md](../../loading-screen/REPORT.md) (lines 31, 43). Its source is not on disk; the files
  below were read from GitHub at that commit (`src/nouveau/horizon/*`, `src/nouveau/vulkan/nvkmd/switch/*` and the NVK
  files named). The build image itself was not opened: the Docker engine was not running.
- `carafe-lab/external/mesa@f97d7dc` is upstream Mesa 25.2.8 with no Horizon winsys: a proxy, used only to check that
  NVK's generic code matches.
- libnx `146c3d14` (pinned with mesa-switch), read from GitHub. DXVK `b1a1c99` (3.1.1) in
  `carafe/runtime/build/dxvk-src`. Autorun `636913ca` in `carafe/external/autorun`; `dlls/win32u/vulkan.c` is identical
  at `a54d5964`. Atmosphère `6b0c077`. Eden `58c1e20` (HLE, secondary).

### The memory types

- NVK on a Tegra SoC: one heap, flagged device-local, and two types: `DEVICE_LOCAL | HOST_VISIBLE | HOST_CACHED` and
  `DEVICE_LOCAL | HOST_VISIBLE | HOST_COHERENT` (`mesa-switch:src/nouveau/vulkan/nvk_physical_device.c:1676-1696`; the
  comment reads "The only difference in memory types is between cached and uncached (but coherent) maps"). The winsys
  reports no VRAM and no BAR, and a non-coherent atom of 128 bytes
  (`src/nouveau/horizon/nouveau_horizon_device.c:52-61`; `nonCoherentAtomSize`, `nvk_physical_device.c:1012`).
- The console's DXVK log shows exactly this: `Heap 0: 1.50 GiB (DEVICE_LOCAL)`, `Type 0: DEVICE_LOCAL | HOST_VISIBLE |
  HOST_CACHED`, `Type 1: DEVICE_LOCAL | HOST_VISIBLE | HOST_COHERENT`, buffer type mask `0x3`
  (`logs/SPEED2.log:397-401, 665`). Autorun does not filter the types (`dlls/win32u/vulkan.c`, no edit of
  `memoryTypes`).

### From a Vulkan allocation to an NvMap and a CPU attribute

- NVK turns `HOST_COHERENT` into `NVKMD_MEM_COHERENT` (`nvk_device_memory.c:55-79`). The Switch backend turns "not
  coherent" into CPU-cached, and GPU-cached unless asked otherwise (`nvkmd/switch/nvkmd_switch_dev.c:751-760`). Every
  allocation is CPU-visible and pitch-kind (`:786-803`).
- The backing is `memalign` heap memory, then `nvMapCreate` (`nouveau_horizon_memory.c:590-606`). For uncached memory
  libnx cleans and invalidates the range and calls `svcSetMemoryAttribute(addr, size, 8, 8)`
  (`libnx:nx/source/nvidia/map.c:55-61`); mesa-switch repeats the call to check its result
  (`nouveau_horizon_memory.c:479-495`). New and recycled memory is zeroed, and cleaned only if cached (`:641-655`).
  A cache of whole backings avoids "kernel cache-attribute transitions" on reuse (`:269-271`).
- `vkMapMemory` returns that same pointer (`nvkmd_switch_dev.c:540-565`, `nouveau_horizon_memory.c:906-920`). The
  32-bit game sees it directly: Autorun uses "native driver mappings" when the process lives in the 32-bit address
  space (`dlls/win32u/vulkan.c:397-406`), logged as `[NXVK] Vulkan memory uses native driver mappings` and
  `driver mapping 0x… for 4194304 / 8388608 / 16777216 bytes` (`logs/autorun_runtime.log:728, 747-791`).
- Flush and invalidate exist only for cached memory: clean (`armDCacheClean`) to the GPU, invalidate
  (`svcInvalidateProcessDataCache` if the syscall is granted, else clean+invalidate) from the GPU, over 128-byte atoms
  (`nouveau_horizon_memory.c:922-1010`; contract in `nouveau_horizon.h:549-556`: "Coherent clients should use
  CPU-uncached memory"). The GPU does not snoop CPU caches; that is why the cached type is not coherent.
- On the GPU side the mapping is GPU-cacheable (`NvMapBufferFlags_IsCacheable`) unless `NVKMD_MEM_GPU_UNCACHED`
  (`nouveau_horizon_memory.c:1153-1158`, `nouveau_horizon_vm.c:379-394`).
- NvMap's own `Alloc` flag: libnx passes 1 for cacheable and 0 for uncached (`map.c:55-56`); Switchbrew calls the
  field "0=read-only, 1=read-write" (NV services page); Eden's HLE reads bit 0 as `map_uncached`
  (`eden:src/core/hle/service/nvdrv/core/nvmap.h:61-67`). The sources disagree. What decides the CPU's view is the page
  attribute set by `svcSetMemoryAttribute`, not this flag. Not verified what nvservices does with it.

### What "Uncached" is on the A57

- `svcSetMemoryAttribute` with the `Uncached` bit rewrites the pages with `OperationType_ChangePermissionsAndRefreshAndFlush`
  (`ams:libraries/libmesosphere/source/kern_k_page_table_base.cpp:1735-1775`). An uncached user page gets
  `PageAttribute_NormalMemoryNotCacheable` (`libraries/libmesosphere/include/mesosphere/arch/arm64/kern_k_page_table.hpp:133-137`),
  index 3 (`kern_k_page_table_entry.hpp:60-63`). MAIR is `0x0000000044FF0400` (`mesosphere/kernel_ldr/source/kern_init_loader.cpp:111`):
  index 3 is `0x44`, Normal memory, inner and outer non-cacheable. Not Device memory: loads may be speculative and
  reordered, stores may be merged.
- What the A57 does with Normal Non-cacheable accesses is not in the optimisation guide (`sources/cortex-a57-optimization-guide.pdf`,
  sections 4.5 and 4.7 cover only load/store throughput and the non-temporal hint). Other ARM TRMs (Cortex-R5 r1p1)
  describe store-buffer merging of non-cacheable Normal stores into one burst per line; that the A57 merges the same
  way is not verified.
- FEX turns the x86 non-temporal stores DXVK uses for constants (`_mm_stream_ps`,
  `src/d3d9/d3d9_constant_copy.cpp:305-317`) into plain `str q` on a core without SVE
  (`runtime/build/fex-src@395b132:FEXCore/Source/Interface/Core/JIT/MemoryOps.cpp:2504-2528`); TSO emulation is off
  (`logs/autorun_runtime.log:80, 782`), so these are ordinary stores, no barriers.

### What DXVK picks

DXVK builds a table from required flags to types; a request with no match falls back: "If there is no cached
coherent memory type, reuse the uncached one. This is likely slow, but API front-ends are relying on it"
(`src/dxvk/dxvk_memory.cpp:2154-2214`, quote at 2181-2182). The cached-memory preference exists only for tilers, and
NVK is not on that list (`src/dxvk/dxvk_device.cpp:723-739`; even then it maps to the same fallback). DXVK never asks
for non-coherent memory, so type 0 is never used.

| Resource | Flags DXVK asks for | Type on the console | Source |
|---|---|---|---|
| Dynamic `D3DPOOL_DEFAULT` VB/IB, `WRITEONLY` (mapped directly) | host-visible, coherent, device-local | 1, uncached | `src/d3d9/d3d9_common_buffer.cpp:83-94, 121-137` |
| Same without `WRITEONLY` ("we might end up reading from it") | + host-cached | 1, uncached | `:128-138` |
| Static or managed VB/IB: staging the game writes | visible, coherent, cached | 1, uncached | `:151-173` |
| UP ring for DrawPrimitiveUP (1 MB, renamed when full) | visible, coherent, device-local | 1, uncached | `src/d3d9/d3d9_device.cpp:4832-4887` |
| Shader constants | visible, coherent (+cached for four small kinds) | 1, uncached | `src/d3d9/d3d9_constant_buffer.cpp:236-252` |
| DXVK staging (texture uploads) | visible, coherent | 1, uncached | `src/dxvk/dxvk_staging.cpp:28-48` |
| Texture mapping buffers (readback, `DEFAULT`-pool locks) | visible, coherent, cached | 1, uncached | `src/d3d9/d3d9_common_texture.cpp:340-345` |
| Managed, systemmem, scratch textures in 32-bit DXVK | none: a CPU-side memory file | cached CPU memory | `d3d9_common_texture.cpp:617-629`, `src/util/util_unmap.h:12-14` |
| NVK push buffers, upload streams | — | uncached by default (`NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=1`) | `mesa-switch:src/nouveau/vulkan/nvk_device.c:285-292`, `nvk_cmd_pool.c:24-27`, `nvk_mem_stream.c:50-52` |
| NVK query pools | — | CPU-cached, GPU-uncached, invalidate before read | `nvk_query_pool.c:168-174, 254-275` |

### What the CPU touches every frame

From the race reports (`logs/autorun_runtime.log`, `[D3D9 calls]`, e.g. 1329, 1736):

- Writes, main thread: up to 260 draws from memory, each a `memcpy` plus zero padding into the UP ring
  (`d3d9_device.h:1360-1375`); constants for every draw with dirty constants, the whole footprint of the shader's
  constants, padded "so we always write full cache lines" (`d3d9_device.cpp:6068-6160`, 6128), 1400–2240 vertex shader
  constant sets a frame; buffer locks, 4–250 a frame, writes by the game into the mapped pointer. Bytes per frame are not
  counted; an estimate of 1–7 MB a frame (50–350 MB/s at 50 fps) assumes 0.3–4 KB of constants per draw and small UP
  draws (not verified).
- Writes, `dxvk-cs`: NVK's push buffers and descriptor data, through the native driver.
- Reads: none by DXVK or NVK per frame. The main thread's `ucrtbase.dll` share, where DXVK's out-of-line `memcpy` goes
  (the 32-bit `d3d9.dll` imports `memcpy` from the CRT), is 0.2–0.3 % (`logs/autorun_runtime.log`, `[PROF] 4w x86 by
  module`); the constant copy is an inlined loop inside `d3d9.dll`'s 17 %.

### What it costs on the A57

- Writes (estimate): with merging, sequential 16-byte stores of whole lines reach the bus as full-line bursts and cost
  about what cached stores cost; partial lines and scattered small stores (NVK's 4-byte push writes) each take a
  store-buffer slot and may drain one by one. DXVK itself warns that "on tilers … uncached stores are expected to be
  very slow" (`dxvk_memory.cpp:2206-2208`). Not measured.
- Reads (estimate from the `[A57]` latency): a dependent load from uncached memory waits for DRAM, at least 377
  cycles (`logs/autorun_runtime.log:838`; the 4 MB chase is partly L2 hits, so the true DRAM figure is higher). A bulk
  read gets no line fill and no prefetch: with 4–8 loads of 8–16 bytes in flight at about 200 ns each it moves
  0.15–0.65 GB/s, against several GB/s from cached memory. A 1280×720×4 readback (3.7 MB) would then take 6–25 ms of
  the main thread instead of 1–2.
- Allocation: libnx cleans the whole range before making it uncached (`map.c:58-61`); for a 16 MB DXVK chunk that is
  262 144 lines once. The BO cache keeps this off the frame.

## What this means for the orchestrator

- There is no cached-coherent memory on this stack. Anything the CPU reads back from GPU-visible memory pays DRAM
  latency per access; anything it writes should be written in whole, sequential 64-byte lines. The orchestrator's own
  shared structures (counters, queues between its threads) must stay in ordinary heap memory, never in Vulkan memory.
- Per game, the decision is driven by what the game reads: if it reads locked buffers or reads back render targets
  every frame, a DXVK patch that gives staging, non-`WRITEONLY` buffers and texture mapping buffers type 0 (cached) with
  `vkFlushMappedMemoryRanges`/`vkInvalidateMappedMemoryRanges` on the dirty range at Unlock and after readback copies
  turns each read into a cached read; the clean of the dirty range costs about what the uncached writes cost now. Behind
  an Experimental row, off by default. Gain: the readback time above (6–25 ms → 1–2 ms per 3.7 MB) for games that do it;
  zero for games that do not.
- NVK's own choice is a switch already: `NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=0` makes push buffers cached with a clean at
  flush. Which is faster for `dxvk-cs` (4-byte push writes to uncached memory against a clean per submit) is a console
  A/B, set from the overlay with `setenv` before the Vulkan device is made.
- If uncached stores turn out slow (more than about 2× cached for full lines), DXVK's per-draw constant copy is the
  first place to look: it rewrites the shader's whole constant footprint on every dirty draw.

## Needs the console

| Measurement | How, from user mode | Decision it switches |
|---|---|---|
| `[A57] uc-write`: cycles per 64 bytes for sequential `stp q` and for `str w` (4-byte, NVK-style) into 1 MB made uncached, against the same buffer cached | `aligned_alloc(4096)`, `svcSetMemoryAttribute(p, n, 8, 8)` as libnx does, the `BENCH` loop style of `carafe_diag.c`, `svcSetMemoryAttribute(p, n, 8, 0)` before `free` | whether CPU-written GPU memory stays uncached (DXVK constants, UP ring, NVK push) or moves to cached+clean |
| `[A57] uc-read`: dependent chase and independent sequential 16-byte loads over 1 MB uncached | same buffer, `chaseCycles`-style chase and a streaming sum | whether the cached-staging DXVK patch is worth building; the readback cost model |
| `[A57] dc-clean`: cycles per line of `armDCacheClean` over 64 KB and 1 MB of dirty cached data | write the buffer, time the clean | the price of the cached+clean alternative |
| NVK push memory A/B: `dxvk-cs` busy share and fps with `NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=0` and `=1` | overlay `setenv` before device creation; existing `[THREADS]`, `[FPS]` | NVK's push memory policy |
| Lock and readback counters in `[D3D9 calls]` (DXVK patch `0005`): locks by `DISCARD`/`NOOVERWRITE`/none, by `WRITEONLY` or not, bytes locked; UP bytes; constant bytes uploaded; `GetRenderTargetData`, `LockRect` on `DEFAULT`-pool surfaces | counters next to the existing ones, per 600 frames | whether a game reads uncached memory per frame; feeds the per-game profile |
