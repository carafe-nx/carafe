# Page sizes (C5) and memory layout: large pages, hot data, false sharing (W8)

## Answer

The kernel picks the page size itself, for every mapping, from alignment alone: it maps with 1 GB, 32 MB, 2 MB,
64 KB or 4 KB entries wherever the virtual address and the physical address are aligned alike and the range is long
enough, and merges neighbours afterwards. There is no system call that asks for a page size. User mode gets large
pages by choosing addresses: a virtual address congruent with the physical pages behind it modulo 2 MB (or 64 KB) and
a size that covers whole blocks. The heap from `svcSetHeapSize` is 2 MB-aligned and filled largest-block-first, so it
is most likely mapped with 2 MB or larger entries. Aliases (`svcMapProcessCodeMemory`, `svcMapMemory`, code memory)
inherit large entries only when the destination is congruent with the source. Two of Carafe's big mappings are not:
the runtime image is mapped at `0x2186000`, which is not even 64 KB-congruent with its 2 MB-aligned source, so its
82 MB use 4 KB entries; and Wine backs guest memory with 4 KB-granular pieces of 2 MB arenas placed at 64 KB-aligned
guest addresses, so most guest data uses 4 KB entries. FEX's code arenas are 2 MB-aligned on both sides and should
get 2 MB entries. The A57's TLBs reach 128 KB (L1, 32 entries) and 4 MB (L2, 1024 entries) with 4 KB pages, 2 MB and
64 MB with 64 KB pages, and up to 2 GB in the L2 with 2 MB pages. The `[A57]` test labelled "TLB misses" did not
measure TLB misses: its buffer sits in the heap, likely 2 MB-mapped, and its lines miss the L2 by construction, so its
356 cycles are DRAM; the cost of a page walk is not known yet. Large pages for the game's data are feasible in Wine's
mapping code and are worth an estimated 1–6 % of the main thread's time if its data overflows the 4 MB TLB reach; the
PMU or the trace of T1 decides.

## Evidence

Sources: Atmosphère `6b0c077` (paths relative to `libraries/libmesosphere`), Autorun `636913ca`
(`carafe/external/autorun`), Carafe tree, FEX checkout `carafe/runtime/build/fex-src` at `395b132`, console log
`D:\debug_switch\carafe\nfsu2-exp9\logs\autorun_runtime.log`.

### What the kernel maps with

- Entry sizes: 4 KB, 64 KB (16 contiguous 4 KB entries), 2 MB, 32 MB (16 contiguous 2 MB entries), 1 GB
  (`include/mesosphere/arch/arm64/kern_k_page_table_entry.hpp:23-31`; the ladder of alignments
  `include/mesosphere/arch/arm64/kern_k_page_table.hpp:49-96`; on the NX board a 4 MB step exists for the SMMU,
  `:52-60`).
- Mapping a physically contiguous range (`MapContiguous`, `source/arch/arm64/kern_k_page_table.cpp:581-650`): below
  64 KB it maps 4 KB entries; otherwise it climbs the ladder while `virt % align == phys % align`, maps the
  unaligned head with smaller entries, the body with the largest common alignment, then steps down for the tail, and
  finally calls `MergePages` (`:641-642`).
- Mapping a page group (`MapGroup`, `:652-742`) does the same per physical block, lowering the alignment until both
  sides agree (`:678-727`), then merges (`:736-737`).
- Changing permissions or attributes of part of a block splits it first (`SeparatePages`, `:821-860`); merging is
  attempted again afterwards (`:961-967,1029`).
- Process switch writes TTBR0 with the process's ASID; no TLB flush (`include/mesosphere/arch/arm64/kern_cpu.hpp:88-92`,
  `include/mesosphere/kern_k_process.hpp:381-390`). System processes on the same core compete for TLB entries but do
  not flush ours.

### Heap, physical memory, code

- `svcSetHeapSize`: size in 2 MB units (Switchbrew SVC page), region start 2 MB-aligned
  (`include/mesosphere/kern_k_page_table_base.hpp:114`, ASLR offsets in those units, `source/kern_k_page_table_base.cpp:322`).
  The extension is allocated with `AllocateAndOpen(…, align_pages = 1, …)` and mapped with `MapGroup`
  (`source/kern_k_page_table_base.cpp:1846-1848,1879-1882`).
- The page allocator takes the largest free block that fits first and goes down: 1 GB, 512 MB, 32 MB, 4 MB, 2 MB,
  64 KB, 4 KB (`source/kern_k_memory_manager.cpp:250-304,306-321`; sizes `include/mesosphere/kern_k_page_heap.hpp:24`).
  Blocks of a buddy-style heap are aligned to their size. With the application pool mostly free at start, the heap is
  expected to be mapped with 2 MB and 32 MB entries. Not verified on the console.
- `svcMapPhysicalMemory` allocates the same way (`AllocateForProcess`, `kern_k_page_table_base.cpp:4426`) and maps
  with `MapFirstGroup` (`:4591`): large entries if the address in the alias region is 2 MB-aligned.
- `svcMapProcessCodeMemory`/`MapCodeMemory` reprotects the source and maps the destination block by block through
  `MapContiguous` (`kern_k_page_table_base.cpp:1002-1060,1347-1367`): the destination gets large entries only where it
  is congruent with the source.
- Both loaders take nearly all memory as heap in one call (`carafe/runtime/loader/source/main.c:114-138`,
  `autorun/horizon-wine/hbl/source/main.c:118-142`), then map the NRO from the heap start to a random code address
  (`carafe/runtime/loader/source/main.c:401-402`). The console log shows that address: runtime
  `0x2186000-0x73bf958` (`autorun_runtime.log`, `[PROF] sampler started` line). `0x2186000 % 0x10000 = 0x6000` while
  the source is 2 MB-aligned, so the whole runtime image (text, data, bss, 82 MB) is mapped with 4 KB entries.

### Guest memory under Wine

- Guest pages are aliases of heap pages, mapped with `svcMapProcessCodeMemory` to the guest address
  (`autorun/dlls/ntdll/unix/horizon.c:17442-17460,18140-18165`).
- The source comes from 2 MB arenas, 2 MB-aligned, handed out first-fit in 4 KB pages; requests above 2 MB get their own
  2 MB-aligned allocation (`autorun/dlls/ntdll/unix/horizon_pool.h:40-48,64-138`; `horizon.c:18092-18095`). Guest
  addresses follow Windows' 64 KB allocation granularity. A piece of an arena at page `j` is 64 KB-congruent with a
  64 KB-aligned guest address only when `j % 16 == 0`: most small and medium guest allocations get 4 KB entries; large
  ones get 64 KB entries, and 2 MB entries only if Wine happened to pick a 2 MB-aligned guest address.
- Page-level protection changes split blocks (`horizon.c:18552-18554`, one 4 KB page at a time).
- FEX's code arenas: 16–128 MB, backing `aligned_alloc(2 MB)` from the heap, RW and RX views both 2 MB-aligned, mapped
  as code memory (`autorun/horizon-wine/source/fex_jit.c:16-20,52-90,122`). Translated code, where the main thread
  spends about 90 % of its time, should therefore sit in 2 MB entries. Not verified.
- FEX's per-thread guest register file: `struct alignas(64) CPUState` (`FEXCore/include/FEXCore/Core/CoreState.h:74`).

### The A57's TLBs and caches

- L1 data TLB 32 entries, fully associative, native 4 KB, 64 KB and 1 MB entries; L2 TLB 1024 entries, 4-way, unified
  (Cortex-A57 TRM r1p3, as quoted by ARM's documentation index, https://documentation-service.arm.com/static/5e906b9fc8052b1608760b6b;
  the TRM itself was not read here). L1 instruction TLB 48 entries (same TRM; not verified). The L2 TLB holds 64 KB,
  1 MB, 2 MB, 16 MB, 512 MB and 1 GB entries; an L1 TLB holds a large page as a smaller entry
  (https://github.com/lgeek/arm_tlb_huge_pages, README table and quote from the A15 TRM). That a 2 MB block occupies
  one 1 MB L1 entry on the A57 is an inference, not verified.
- Measured on a 2 GHz A57 (AMD Opteron A1100, Linux, 4 KB pages): L1 TLB miss 7 cycles, L2 TLB miss 30–36 cycles
  (https://www.7-cpu.com/cpu/Cortex-A57.html). That is a walk whose descriptors hit the cache; a descriptor in DRAM
  adds a DRAM access.
- Reach: 4 KB pages 128 KB (L1) / 4 MB (L2); 64 KB pages 2 MB / 64 MB; 2 MB blocks about 32 MB (L1, as 1 MB) / 2 GB.
- Cache line 64 bytes; a load crossing a 64-byte line or a store crossing a 16-byte boundary costs extra
  (`D:\debug_switch\carafe\sources\cortex-a57-optimization-guide.txt:1915-1916`, Optimization Guide §4.6, pages 39–40).

### What the `[A57]` page test measured

- The test allocates its buffer with `aligned_alloc` from the heap (`carafe/runtime/overlay/carafe_diag.c:161-163`)
  and visits one line per 4 KB page at in-page offset `(index % 64) * 64` (`:157-159`).
- With the heap 2 MB-mapped, 16 MB is 8 L2 TLB entries and about 16 L1 entries: probably no TLB misses at all.
- Inside a 2 MB block the physical address bits 6–20 equal the virtual ones. The L2 set index (bits 6–16 for 2 MB,
  16-way, 64-byte lines) is then fixed by `index % 64` alone: 4096 lines fall into 64 of 2048 sets, which hold 1024
  lines. Nearly every visit misses the L2, so the 356 cycles are a DRAM access, close to the 377 of the 4 MB test.
- The conclusion in `carafe/docs/research/game-speed/REPORT.md:246` ("a TLB miss adds nothing over it") does not
  follow. The walk cost is unmeasured; the test below separates it.

### False sharing and shared lines in Carafe's threads

- Counters bumped by every thread: `carafe_lock_boosts` on every `svcArbitrateLock` (`carafe/runtime/overlay/carafe_lock_boost.c:20`),
  `wine_nx_server_ticks[]`/`wine_nx_server_calls[]` on every server request (`autorun/dlls/ntdll/unix/server.c:364,426`).
  Each costs one contended line transfer next to a system call that costs a microsecond: harmless now, worth fixing
  only if they move into a lock-free hot path.
- Intended sharing: the main thread fills DXVK's command chunks, `dxvk-cs` on core 3 reads them. Every line crosses
  cores once; the L2 is shared, so a clean line costs an L2 hit (34 cycles measured) and a line still dirty in the
  writer's L1 an estimated 40–100 cycles. Not verified.
- FEX's guest register files are 64-byte-aligned per thread, so threads do not share their lines.
- The game's own threads: NFSU2 runs one busy thread, so its false sharing does not matter today. It matters the day
  the orchestrator splits the game's own work across cores (W6): the guest's layout is the game's, so split work along
  64-byte-aligned ranges.

## What this means for the orchestrator

- Large pages are an address choice, made in three places Carafe controls:
  1. Carafe's loader (ISC, `runtime/loader/source/main.c:401`): map the NRO at an address congruent with the heap start
     modulo 2 MB. The runtime's text then gets 2 MB entries over its interior. Helps native threads (`dxvk-cs`,
     `dxvk-submit`, Mesa's paths), little for the main thread, which runs translated code. Cheap.
  2. Wine's guest mapping (`horizon.c`, patch): for reservations of 2 MB and more, pick a 2 MB-aligned guest address
     (legal under Windows' 64 KB granularity) and a dedicated 2 MB-aligned source; for smaller ones, pick arena pages
     congruent with the guest address modulo 64 KB and round to 64 KB. Gives 2 MB or 64 KB entries to the game's data.
     Committing a reservation piece by piece then relies on `MergePages` joining neighbours; whether it does across
     separate `svcMapProcessCodeMemory` calls is not verified.
  3. FEX's arenas already do it; keep it so.
- Keep page-granular protection changes away from large regions: a guard page in the middle of a heap splits a 2 MB
  block into 512 entries. Put guards at region edges.
- Gain on NFSU2, with the reasoning: a frame at 47 fps is about 40 M cycles of the main thread. If its data overflows
  4 MB and, say, 2 % of its loads miss the L1 TLB (7 cycles each) and 0.2 % miss the L2 TLB (30–36 cycles, more if the
  descriptor is in DRAM), with about 10 M memory accesses a frame, that is 1.4 M + 0.7 M cycles, about 5 %. With a
  working set inside 4 MB it is under 1 %. Range 1–6 %, decided by `tlb-ladder` and the PMU's `L1D_TLB_REFILL`
  ([pmu.md](pmu.md)) or the access trace of T1.
- Hot data: the translator cannot move the game's objects (raw pointers), but it can choose which physical memory,
  page size and cache sets back them, and it can prefetch (W5, W7). A helper thread's prefetches on any core land in
  the shared L2.
- False sharing among our own threads is not a measurable cost today. Rules for new code: per-thread counters,
  64-byte-aligned queue heads and tails on separate lines, batch hand-offs by whole lines.

## Needs the console

| Name | How to measure in `[A57]` (user mode) | Decision it switches |
|---|---|---|
| `tlb-ladder` | One line per 4 KB page over N pages, N = 16, 32, 64, 512, 1024, 2048, 4096, 16384, at in-page offset `((i >> 5) & 63) * 64` so that up to 2048 lines take distinct L2 sets (they stay L2-resident; up to 512 stay L1-resident). Run twice: (a) on a heap buffer; (b) on the same pages aliased with `svcMapProcessCodeMemory` to a destination offset by 4 KB from congruence, which forces 4 KB entries. The difference at N > 32 is the L1 TLB miss; at N > 1024 the walk with cached descriptors; at N = 16384 the walk with descriptors partly in DRAM. | Whether large guest pages are worth a Wine patch: do it if the walk costs 25 cycles or more and the PMU or trace shows the main thread over 4 MB of pages. |
| `heap-block-probe` | From (a): if N = 2048 and 4096 cost the same as N = 1024, the heap is block-mapped. | Whether the native heap already has large pages, and whether `tlb-ladder` (a) is a valid baseline. |
| `alias-64k` | As (b), destination 64 KB-congruent; and a 2 MB-congruent destination. | Whether 64 KB entries give most of the gain (they need no change of guest addresses), or 2 MB is needed. |
| `merge-probe` | Map a 2 MB-aligned source to a 2 MB-aligned destination in 32 separate 64 KB `svcMapProcessCodeMemory` calls, then run the ladder on it. | Whether committing guest memory piece by piece can still end in 2 MB entries. |
| `handoff-lines` | See `line-pingpong` in [horizon-scheduler.md](horizon-scheduler.md): 16 KB written on core 1, read on core 3, cycles per line. | Batch size for any stage that moves data between cores. |
