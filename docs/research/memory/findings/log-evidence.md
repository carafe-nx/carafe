# Memory on the Switch: what the logs we already have show

Research snapshot: 2026-11-05. For subsequent runtime changes and validation, see the
[report](../REPORT.md#updates-after-the-study).

Research note, 2026-11-05. Read-only study of the console logs in `console-logs/` and the lab runs in
`lab-runs/`. Nothing was built or run.

Every statement is marked **[fact]** (with the file and line or the log line) or **[hypothesis]**. Line numbers
for `.log.gz` files are line numbers of the decompressed log.

## 1. Sources and method

- Console: 355 `autorun_runtime*.log` files under `console-logs/` (each folder holds the live log and up to
  five rotated ones, and several folders hold copies of the same rotated logs). The games are Cat Mail Co
  (`catmail-2026-11-05`), OpenTTD (`0563ab630a362000`), NFS Underground (`2026-10-14-frames*`, `Speed.exe`), NFSU2
  (`nfsu2-*`), NFS Carbon (`nfsc-*`, `sarek-nfsc*`), Dead Space (`sarek-ds*`) and Undertale. There is **no console log
  of Fields of Mistria** in that folder. No console log has `c0000017` or `NO_MEMORY`.
- Lab: 911 `autorun_runtime.log.gz` and 87 plain logs. Most runs that passed have no logs left (the logs folder is
  empty): only failed or warned launches keep theirs. `summary.json`'s `peakMemoryMb` is the **Docker container's**
  memory (the whole Eden process, including the emulated Switch's DRAM), not the game's heap
  (`carafe-lab/app/crates/lab-core/src/resources.rs:14-17, 61-72`), so it says nothing about the guest. It is around
  4.4 GB for every Cat Mail Co run whatever the game did.
- Every log was summarised with one awk pass: the `[PROGRESS]` sample with the highest `heap_used_mb`, the
  `[MEMPOOL]` line that follows it, the largest `[THREADS]` count, `[BUDGET] threads=`, `[INIT] memory=`, and counts of
  `create_backing failed`, `[VAFAIL]` and `errno=12`.

## 2. What the counters mean (read from the code)

These definitions decide what the numbers can and cannot explain.

| Counter | Meaning | Source |
| --- | --- | --- |
| `heap_used_mb` | newlib `mallinfo().uordblks`: bytes in use in the libnx heap. The comment says the heap backs "Wine's guest memory, the GPU's buffers and translated code" | `external/autorun/horizon-wine/source/runtime.c:1318-1349` |
| `heap_free_mb` | free chunks inside malloc's arena plus the part of the heap malloc has not taken yet. **Not** the largest free block | same, `:1325` |
| `[INIT] memory=` | `InfoType_TotalMemorySize` of the process | `dlls/ntdll/unix/horizon.c:3903` |
| `arena_mb` | live 2 MB arenas of the two page pools (guest backings of up to 2 MB, and section pages), times 2 | `horizon.c:3799-3809`, `horizon_pool.h:39-47` |
| `block_allocs` | **cumulative count** of dedicated allocations: any backing larger than one arena, rounded up to a multiple of 2 MB with `aligned_alloc(2 MB, …)`. Their size is never counted | `horizon_pool.h:117-129` |
| `shared_allocs` | cumulative count of fallbacks to a 4 KB-aligned allocation from the general heap, taken when the 2 MB-aligned one failed | `horizon_pool.h:131-138` |
| `backing_slots`, `mapping_slots` | **high-water mark** of the static slot arrays (4096 and 8192). Past them objects come from `calloc`; the figure never goes down and is not a limit | `horizon.c:3638-3641`, `horizon_pool.h:16-24` |
| `section_views` | cumulative count of views mapped from sections without a file | `horizon.c:3670-3672` |
| `code_mb`, `code_all_mb` | Box64's dynarec only. FEX builds print `0/0` (only one old run, `nfsu2-exp/logs/autorun_runtime.5.log`, shows `code=20/48`) | `runtime.c:1228-1263` |
| `cache_mb` | the SD-card read cache | `runtime.c:1327-1330` |
| `[NXVK] driver mapping` | host-visible Vulkan memory mapped by the driver: **only the first 8** are logged | `dlls/win32u/vulkan.c:1717-1720` |
| `[MEM] after X: N regions lent away` | heap regions that are borrowed, IPC-mapped or device-mapped at that moment | `runtime.c:2691-2703` |

Two code facts matter for how much a game costs:

- **[fact]** Every committed guest range gets its own heap copy at commit time: the backing is allocated, zeroed with
  `memset`, and for a file mapping the whole file range is read into it (`horizon.c:18076-18135`). Nothing is committed
  lazily and file mappings are copies, not shared pages.
- **[fact]** A backing larger than 2 MB is rounded up to the next multiple of 2 MB (`horizon_pool.h:125-126`): a
  `0x210000` (2.06 MB) commit takes 4 MB of heap.

## 3. Heap size per address space

| Build | `[MAP] heap region` | `[INIT] memory=` | usable heap (used + free) | Source |
| --- | --- | --- | --- | --- |
| 32-bit games (`address space 32 bits`) | 2 GiB | 2048 MB console and lab | about **1849 MB** | `nfsu2-2026-11-04` line 5 ff. |
| 64-bit games (`address space 39 bits`) | 8 GiB of address space | 3189 MB console, 3285 MB lab | about **3082 MB** console, **3081 MB** lab | `catmail-2026-11-05/logs/autorun_runtime.log:5,66,382` |

- **[fact]** On the console the read-write heap is one block of 3070 MB at start-up: `[MAP] start-up:
  42e8403000-43a8210000 heap perm=3` (Cat Mail Co, line 17).
- **[fact]** About 100 MB of the heap is lent away before the game starts: `[MEM] after the network: 7 regions lent
  away, 102080 KB` (Cat Mail Co line 34; 96–102 MB on every console log, 94 MB in the lab).
- **[fact]** A trivial 64-bit program already holds 230–300 MB of heap in the lab: Message_probe 232 MB, Sockets_probe
  275 MB, Race_probe 303 MB, `probe-input-keyboard-x86` 300 MB.

## 4. Peak memory per game

`Peak` is the highest `heap_used_mb` sample; `free` is from the same sample; the `[MEMPOOL]` figures are from the line
after it. `Threads` is the largest `[THREADS] N threads` and, after the slash, the largest `[BUDGET] threads=` (Wine's
whole count). `code_mb` is `0/0` in every row (FEX builds do not count translated code).

### Console

| Game (folder) | Bits | Peak used MB | Free MB | arena MB | block allocs | backing / mapping slots | Threads | Out of memory |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Cat Mail Co (`catmail-2026-11-05`) | 64 | **3030** at 86 s | **52** | 280 | 295 | 1075 / 3193 | 50 / 78 | **yes**, "new game" |
| OpenTTD (`0563ab630a362000`) | 64 | 1022 at 15 s | 2066 | 96 | – | 483 / 1418 | 42 | no |
| NFS Underground (`2026-10-14-frames*`) | 32 | 1074–1173 | 679–778 | 100–114 | – | 458–562 / 2185–2872 | 34 | no |
| NFSU2 (`nfsu2-*`, 29 folders) | 32 | 1145–1283 (`nfsu2-slow`) | 567–706 | 128–166 | – | 662–927 / 3608–5077 | 20–23 / 45–54 | no |
| NFS Carbon (`nfsc-exp*`, `sarek-nfsc2`) | 32 | 1218–1271 (`nfsc-exp13`) | 577–630 | 134–146 | – | 676–760 / 2849–3370 | 22–42 / 47–67 | no |
| Dead Space (`sarek-ds1`) | 32 | 1167 | 685 | 98 | – | 480 / 1451 | 38 | no |
| Undertale (`undertale-tso-2026-11-04`) | 32 | 689 | 1156 | 118 | – | 437 / 1264 | 16 / 39 | no |

The NFS folders show `[VAFAIL]` lines (8–11 per log), but all of them are `status=0xc0000018` (conflicting addresses)
for fixed bases such as `0x10000000`, with no `errno=12`: address clashes, not lack of memory
(`nfsu2-2026-11-04/logs/autorun_runtime.log`).

### Lab (Eden)

| Game (run with the highest peak) | Bits | Peak used MB | Free MB | arena MB | backing / mapping slots | Threads | Out of memory |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Fields of Mistria (`2026-10-31_18-23-05`) | 64 | **3059** at 15 s | **25** | 894 | 4096 / 8192 | 19 | **yes, 10 of 14 launches** |
| Megabonk (`2026-11-04_07-18-56` launch-1) | 64 | **2941** at 40 s | **141** | 282 | 942 / 2915 | 50 / 75 | no (log ends) |
| Dark Souls II (`2026-10-31_18-23-05`) | 64 | 2558 | 527 | 264 | 634 / 1939 | 38 / 65 | no |
| Pizza Tower (`2026-11-04_08-03-05`) | 64 | 2497 | 585 | 204 | 972 / 2083 | 24 / 46 | no |
| Cat Mail Co (`2026-11-03_16-35-47` B) | 64 | 2414 at the menu | 667 | 250 | 789 / 2691 | 50 / 79 | not in surviving logs, see 5.3 |
| Dishonored (`2026-11-05_02-05-29`) | 32-bit game, 39-bit build | 1423 | 1659 | 366 | 3002 / 5258 | 42 / 68 | no |
| Undertale Yellow | 64 | 1291 | 1791 | 152 | 547 / 1513 | 18 / 41 | no |
| Spore | 64 | 1270 | 1815 | 210 | 991 / 2820 | 25 / 51 | no |
| NFS Carbon | 32 | 1233 | 615 | 144 | 733 / 2928 | 25 / 50 | no |
| A Short Hike | 64 | 1168 | 1914 | 268 | 1248 / 3042 | 52 / 74 | no |
| Wolverine | 32 | 1022 | 826 | 312 | 3166 / 5364 | 21 / 46 | no |
| OpenTTD | 64 | 1040 | 2048 | 94 | 469 / 1293 | 24 / 49 | no |
| Dead Space | 32 | 988 | 860 | 118 | 515 / 1796 | 24 / 49 | no |
| NFS Underground | 32 | 933 | 918 | 92 | 420 / 2054 | 20 / 45 | no |
| Street Racing Syndicate | 32 | 883 | 966 | 102 | 425 / 2182 | 18 / 45 | no |
| Indiana Jones | 32 | 837 | 1011 | 144 | 974 / 2682 | 19 / 46 | no |
| Undertale | 32 | 815 | 1033 | 98 | 380 / 1045 | 9 / 36 | no |
| Antichamber | 64 | 779 | 2302 | 124 | 684 / 2620 | 17 / 42 | no |
| HoMM IV | 32 | 655 | 1197 | 58 | 255 / 1787 | 8 / 34 | no |
| Hitman 2 | 32 | 1660 once (2026-10-31), 550 since | 189 | 54 | 202 / 855 | 17 / 42 | no |
| NFSU2 (`2026-10-29_19-37-54` A launch-2) | 32 | **1812** | **36** | 98 | 406 / 2005 | 21 / 47 | **yes, after a GPU device loss**, see 7 |

**[fact]** The only real games that ran out of memory in the logs are Cat Mail Co (console and lab) and Fields of
Mistria (lab), both 64-bit, both at about 3.03 GB of a 3.08 GB heap. Megabonk reached 2.94 GB with 141 MB free. No
32-bit game came near its 1.85 GB in normal play.

## 5. Cat Mail Co

### 5.1 Console: heap over time

`console-logs/catmail-2026-11-05/logs/autorun_runtime.log`. The memory note for this run says the main menu
showed at about 51 s (the overlay hid the loading screen "after 51 s: Vulkan frames") and "new game" was pressed from
it.

| Time | Line | used MB | free MB | arena MB | block allocs (cum.) | shared allocs | backing / mapping slots | Event |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 5 s | 382 | 1003 | 2079 | 134 | 72 | 0 | 406 / 1461 | start-up, before Vulkan |
| 15 s | 1116 | 1844 | 1237 | 220 | 138 | 0 | 710 / 2256 | Vulkan up, splash: **+841 MB in 10 s** |
| 25 s | 1124 | 1876 | 1205 | 220 | 141 | 0 | 710 / 2256 | |
| 35 s | 1584 | 1914 | 1167 | 220 | 143 | 0 | 710 / 2256 | |
| 45 s | 2097 | 2019 | 1062 | 240 | 158 | 0 | 737 / 2450 | |
| 56 s | 2193 | 2223 | 859 | 248 | 185 | 0 | 797 / 2624 | main menu |
| 66 s | 2229 | 2496 | 585 | 254 | 233 | 0 | 869 / 2848 | loading the new game |
| 76 s | 2305 | 2606 | 475 | 262 (peak 266) | 253 | 0 | 897 / 2943 | `writes_held` jumps 432 → 6160 |
| 86 s | 2367 | **3030** | **52** | 280 | 295 | **72** | 1075 / 3193 | failure between 76 s and 86 s |
| 96 s | 2414 | 3030 | 52 | 280 | 295 | 72 | same | |
| 106–126 s | 2420–2435 | 2942 | 140 | 280 | 295 | 72 | same | PreloadManager at 99.5 % of a core |
| 136 s | 2441 | 2918 | 164 | | | | | |
| 147 s | 2449 | 2870 | 212 | | | | | last sample |

- **[fact]** From the menu to the failure the heap grew by about 800 MB (2223 → 3030) while `arena_mb` grew by 32 MB
  and `block_allocs` by 110.
- **[fact]** `shared_allocs` went from 0 to 72 between 76 s and 86 s: 72 backings could not get a 2 MB-aligned block
  and fell back to 4 KB-aligned general heap.
- **[fact]** Vulkan offered the game a heap far larger than what is left: `Heap 0: 2.33 GiB (DEVICE_LOCAL)`,
  `Budget: 1.37 GiB` (line 396–397; lab: 2.40 GiB and 1.35 GiB). The GPU's memory comes from the same 3 GB heap.
- **[fact]** Unity printed its default allocator setup (lines 164–193): main and gfx allocators in 16 MB blocks, bucket
  and cache allocators in 4 MB blocks, a 4 MB main temp allocator.

### 5.2 Console: the failure sequence

Lines 2309–2363, between the 76 s and the 86 s sample:

1. `[HMAP] create_backing failed addr=0x71f400000 size=0x400000 prot=0x0 errno=12` (2310) — a 4 MB commit on thread
   `00a8` (168, `Loading.PreloadManager`) gets no heap.
2. `[HMAP] commit reservation failed range=0x71f400000/0x400000 prot=0x3 errno=12` (2311).
3. `[VAFAIL] tid=00a8 base=0x71f400000 size=0x400000 type=0x1000 prot=0x4 ... status=0xc0000022 count=1` (2312):
   `NtAllocateVirtualMemory(MEM_COMMIT, PAGE_READWRITE)` returns **`STATUS_ACCESS_DENIED`**. Windows would return
   `STATUS_NO_MEMORY` or `STATUS_COMMITMENT_LIMIT`.
4. `[SEH] raised tid=00a8 code=c0000005 at 0x7ff3f2bbed ... params=2: 1 71f7fffe8` (2313): the thread writes to
   `0x71f7fffe8`, the last bytes of the 4 MB range it failed to commit. **[hypothesis]** It ignored the failed result,
   or the range is a stack-like buffer filled from its top.
5. `dbghelp.dll`, `kernel32.dll`, `psapi.dll` load on the same thread (2314–2328): Unity's crash handler.
6. 28 read-only mappings fail (2329–2361), most of them at the same address `0xea970000` with shrinking sizes
   (`0x230000`, `0x4fc000`, `0x2d0000`, `0x220f000` …), first as one backing, then in 2 MB chunks ("fixed replacement
   failed: map backing chunks"). **[hypothesis]** The crash handler maps module images or data files for a dump.
7. `[VAFAIL] tid=00a8 base=0x7ecbaf1000 size=0x400000 ... status=0xc0000022 count=2` (2363): a second 4 MB commit fails.
8. The process keeps running: frames go on at about 21 fps, `Loading.PreloadManager` spins at 99.5 % and the heap
   slowly shrinks to 2870 MB by 147 s. The game never loads.

### 5.3 Lab reproduction

**[fact]** No surviving lab log of Cat Mail Co contains the failure. The 11 lab logs that survive (2026-11-02 to
2026-11-04) all stop at the main menu, at 2237–2414 MB used and 667–844 MB free (for example
`runs/2026-11-04_08-03-05/Cat_Mail_Co/launch-1`, lines 1108–1504: 1777 → 2074 → 2342 MB). The menu therefore costs
about the same in the lab as on the console (2223 MB at 56 s).

**[fact]** The lab did reproduce the failure on 2026-11-04 around 23:39 UTC (run `2026-11-04_23-34-26`, played by hand,
outcome `skip`), but its log was read only from Eden's SD card
(`build/work/eden/user/sdmc/switch/carafe/05aec267ffcc2000/logs/autorun_runtime.log`), and that folder no longer exists;
the run's own `logs` folder is empty. What remains is in the session transcript
`archived console investigation transcript (2026-11-05)`, lines 2397–2404,
and in the memory note `night-2026-11-05-gamepad.md`:

```
heap_used_mb=2895 heap_free_mb=186            (five samples in a row)
[VAFAIL] tid=00b0 base=0x5fcf400000 size=0x400000 type=0x1000 prot=0x4 zero_bits=0 status=0xc0000022 count=1
[SEH] raised tid=00b0 code=c0000005 at 0x7ff3f2bbed flags=0 params=2: 1 5fcf7fffe8 0
[VAFAIL] tid=00b0 base=0x7dfcf11000 size=0x400000 type=0x1000 prot=0x4 zero_bits=0 status=0xc0000022 count=2
[THREADS] 52 threads use 1.98 cores: 176w:Loading.PreloadManager@0 90.7% ...
```

- **[fact]** It is the same sequence as on the console: a 4 MB commit by `Loading.PreloadManager`, `c0000022`, an
  access violation at the **same instruction** `0x7ff3f2bbed`, writing to the top of the range, then a second 4 MB
  commit failing.
- **[fact]** In the lab the 4 MB commit failed with **186 MB reported free**; on the console the first sample after the
  failure showed 52 MB free.
- **[hypothesis]** The free heap is fragmented: no free piece of 4 MB is left although 186 MB are free in total. The
  72 fallbacks to 4 KB alignment on the console (5.1) point the same way. `heap_free_mb` cannot confirm it because it
  does not report the largest free block.

## 6. Fields of Mistria (lab only)

64-bit, OpenGL (no DXVK). 10 of 14 lab launches end in `errno=12`.

`runs/2026-11-03_00-05-02/Fields_of_Mistria/launch-1/logs/autorun_runtime.log.gz`:

| Time | Line | used MB | free MB | arena MB | block allocs | backing / mapping slots |
| --- | --- | --- | --- | --- | --- | --- |
| 5 s | 578 | 1921 | 1161 | 226 | 218 | 2300 / 3138 |
| 15 s | 614 | 2464 | 618 | 552 | 341 | 4096 / 8192 |
| 25 s | 741 | 3028 | 54 | 988 | 415 | 4096 / 8192 |

Then (lines 745–771):

```
[HMAP] create_backing failed addr=0x64bb560000 size=0x210000 prot=0x0 errno=12
[VAFAIL] tid=0020 base=0x64bb560000 size=0x210000 type=0x1000 prot=0x4 ... status=0xc0000022 count=1
[STDERR] memory allocation of 2097152 bytes failed
[VAFAIL] tid=0028 base=0x3293f60000 size=0x210000 ... count=2
[LIFECYCLE] final tid=32 code=c0000409
[CARAFE] the game failed (kind 0, code 0xc0000409, mistria.exe + 0x938981), 1 in a row
```

- **[fact]** The game's allocator asked for 2 MB (`memory allocation of 2097152 bytes failed`, Rust's message), which
  reached Horizon as a `0x210000` commit, and the process aborted with `c0000409`.
- **[fact]** By the code in section 2, each such `0x210000` commit takes a 4 MB block. **[hypothesis]** If the game
  makes many 2 MB allocations, close to half of what those blocks hold is rounding. The log does not count them.
- **[fact]** Unlike Cat Mail Co, Fields of Mistria fills the 2 MB arenas: `arena_mb` reaches 988–1074 MB. Its
  backings pass 4096 and its mappings 8192 within 15 s (the high-water marks of the static arrays; past them `calloc`
  takes over, so this is not the failure).
- **[fact]** In `runs/2026-11-04_08-03-05/Fields_of_Mistria/launch-2` (lines 578–616) the failure follows a sample with
  848 MB free. **[hypothesis]** The heap ran out between two samples 10 s apart: the other launch grows by 500–560 MB
  per 10 s (1738 → 2445 → 2910 MB in `launch-1`).
- **[fact]** At exit, after the failure, the memory map still shows 138 MB of heap device-mapped (`attr=4`, the GPU)
  and 85.5 MB borrowed (`attr=1`, alias sources) (sum of the `[MAP] exit:` lines of the `2026-11-03_00-05-02` log).

## 7. NFSU2 in the lab: growth after a GPU device loss

`runs/2026-10-29_19-37-54/Need_for_Speed_Underground_2___A/launch-2/logs/autorun_runtime.log.gz`:

- **[fact]** For about 4100 s the heap stays at 688–694 MB with `arena_mb=98`, `backing_slots=406`; `block_allocs`
  rises by about one every 30 s (allocations and frees, no growth) (lines ~2000–24235).
- **[fact]** At about 4150 s the frame count stops at 155837, DXVK reports `VK_ERROR_DEVICE_LOST` again and again and
  "Exception on CS thread!", and the heap grows by about 10 MB/s: 694 → 1225 → 1812 MB at 4281 s (36 MB free)
  (lines 24234–34214). **Every `[MEMPOOL]` figure stays the same** during this growth.
- **[fact]** Then 4 KB-granular 576 KB commits fail (`create_backing failed ... size=0x90000 errno=12`, lines
  34603–34645), the game exits with code 3, and 18641 pages (73 MB) are still with the GPU when the driver sessions
  close (lines 34678–34683).
- **[hypothesis]** The growth is native malloc (DXVK, Mesa or the driver retrying work after the device loss), not
  guest memory, because nothing guest-side moves. It is a runaway after a GPU fault, not a leak in normal play.

## 8. How much of the heap the logs explain

Cat Mail Co on the console at 86 s, 3030 MB used:

| Part | MB | Kind |
| --- | --- | --- |
| Guest backings of up to 2 MB and section pages (`arena_mb`) | 280 | measured |
| Vulkan memory mapped by the driver, first 8–9 mappings only | 76 | lower bound |
| SD-card read cache (`cache_mb`) | 24 | measured |
| Lent away at start (network and early device mappings) | about 100 | measured at start; whether it is inside `heap_used` is not shown |
| Wine and the runtime themselves, from 64-bit probes | about 230–300 | estimate from other runs |
| **Explained** | **about 600–780** | |
| **Not explained by any counter** | **about 2250–2430 (75–80 %)** | |

The same split for other games at their peaks:

| Game | Used MB | arena MB | Vulkan logged MB | cache MB | Unexplained beyond arena + cache + about 250 MB base |
| --- | --- | --- | --- | --- | --- |
| Fields of Mistria (lab) | 3028 | 988 | – (OpenGL) | 15 | about 1770 MB |
| Megabonk (lab) | 2941 | 282 | – | 25 | about 2380 MB |
| NFSU2 (console `nfsu2-2026-11-04`) | 1267 | 158 | 64 | 21 | about 770 MB |
| NFS Carbon (console `nfsc-exp16`) | 1263 | 138 | 96 | 30 | about 750 MB |

What the unexplained part can be, all **[hypothesis]**, because no counter sizes it:

1. **Dedicated blocks**: guest commits larger than 2 MB, including file mappings and DLL images, which are read whole
   into heap copies. Cat Mail Co had made 295 of them by the failure, each at least 4 MB, so at least 1.18 GB was
   allocated this way over the run; how much was still live is unknown. Between the menu and the failure the heap grew
   by 807 MB with 110 new blocks and 32 MB more arena: about 7 MB per block, which fits Unity's 4 and 16 MB blocks.
2. **GPU memory** from DXVK and NVK. Only the first 8 driver mappings are logged, and Vulkan offers 2.3–2.4 GiB to a
   64-bit game and 1.5 GiB to a 32-bit one, out of the same heap.
3. **Commit without touch**: on Windows a committed page costs physical memory only once it is touched. Here
   `create_backing` allocates and zeroes every committed byte at commit time (`horizon.c:18095-18105`). Unity's 16 MB
   allocator blocks and the Mono or IL2CPP GC heap are committed generously. This can make the same game cost much more
   than its Windows working set.
4. **FEX**: its code buffers and per-thread state. FEX builds print no code size (`code_mb=0/0`), and the
   `[FEX] since the last report` lines count blocks, not bytes.
5. Native malloc by Wine, DXVK, Mesa, FMOD and the like, invisible except as part of `heap_used_mb` (section 7 shows it
   can be large).

## 9. Patterns

- **Steady state, no leak in normal play** [fact]:
  - NFSU2 on the console stays at 1006 MB from 125 s to 1810 s and at about 1129 MB from 2051 s to 2774 s
    (`nfsu2-exp17`).
  - NFS Underground runs for 13608 s with a peak of 1053 MB and 998 MB at the end
    (`2026-10-14-frames/0556710511f9c000/logs/autorun_runtime.4.log`).
  - NFSU2 in the lab runs for 4100 s flat at 688 MB before the device loss.
- **Jumps on loads** [fact]:
  - NFSU2 goes 1006 → 1131 MB when a race loads at about 1930–2050 s (`nfsu2-exp17`).
  - NFSC goes 1012 → 1191 → 1233 MB between 65 s and 125 s (`nfsc-exp10`).
  - Cat Mail Co gains +841 MB in the 10 s after Vulkan starts, then +807 MB for "new game".
  - Megabonk goes 1996 → 2941 MB between 26 s and 40 s.
  - Fields of Mistria gains 500–560 MB every 10 s from start-up.
- **Memory is returned** [fact]:
  - OpenTTD drops from 990 to 547 MB (`0563ab630a362000/logs/autorun_runtime.log`).
  - Cat Mail Co drops from 3030 to 2870 MB after its failure.
  - `arena_reclaims` grows in every game (94 in Cat Mail Co).
  - By design up to 8 empty arenas per pool, 32 MB in all, are kept (`horizon_pool.h:41`).
  - There is no sign of guest memory that is never returned after an unmap during a run.
- **Not returned at exit** [fact]: GPU pages stay with the driver when the process ends: 18641 pages in the NFSU2 run
  (section 7), 138 MB device-mapped in the Fields of Mistria exit map.
- **Fragmentation near the limit** [fact + hypothesis]: the 72 `shared_allocs` on the console and the 4 MB failure
  with 186 MB "free" in the lab (section 5) suggest that the last few hundred MB of the heap cannot be used for
  multi-MB blocks.

## 10. Diagnostics that are missing

To attribute the heap fully, the logs would need:

1. **Live bytes in dedicated blocks** next to `block_allocs` (and live, not cumulative, counts). Today the biggest
   guest-memory kind has no size at all.
2. **Guest committed bytes split by kind**: private commits, image mappings, file mappings and section views, in MB.
   Today `backing_slots` and `mapping_slots` are high-water counts of static arrays.
3. **GPU memory**: total bytes device-mapped (`attr=4`) in every `[PROGRESS]`, and DXVK's own allocated or used
   figures. The `device_regions` walk the exit path already does (`runtime.c:2723`) could run every 10 s. The
   `[NXVK] driver mapping` log stops after 8 lines.
4. **The largest free block** of the heap (or a malloc fragmentation figure), to tell "out of memory" from "out of
   contiguous memory". `heap_free_mb` mixes free chunks with the untouched top.
5. **FEX code and thread memory** in bytes. The `code_mb` field only works for Box64.
6. **Rounding waste**: bytes asked for against bytes allocated for blocks (for example Fields of Mistria's `0x210000`
   commits).
7. **A snapshot at the moment of failure**: `[PROGRESS]`, `[MEMPOOL]` and a memory-map summary printed when the first
   `create_backing failed` happens, instead of up to 10 s later.
8. **Commit against touch**: how many committed bytes the guest has actually written. It can only be estimated by
   counting backings that are still all zero, or by comparing with the same game's commit size on Windows.
9. **Lab log retention**: the one lab run that reproduced Cat Mail Co's failure kept no log, because the run was
   stopped by hand. Logs of `skip` launches should be kept like failed ones.

## 11. Facts and hypotheses in short

Facts:

- 64-bit games have about 3082 MB of heap and 32-bit games about 1849 MB, on the console and in the lab alike.
- Cat Mail Co fails at 3030 MB used (console) or 2895 MB used with 186 MB free (lab), on a 4 MB commit by
  `Loading.PreloadManager`. The commit returns `c0000022`, the thread then faults at `0x7ff3f2bbed`, and the game
  spins forever.
- Fields of Mistria fails at about 3030 MB on a 2 MB allocation (`0x210000` commit) and aborts with `c0000409`.
- Of Cat Mail Co's 3030 MB, the counters explain only about 600–780 MB.
- In normal play there is no steady growth. The one large runaway (+1.1 GB) followed a GPU device loss and happened
  outside every guest-memory counter.

Hypotheses (unverified):

- The unexplained heap is dedicated multi-MB blocks, committed but untouched memory, GPU memory and FEX.
- The final failures come from heap fragmentation rather than a truly empty heap.
- Fields of Mistria pays rounding waste on 2 MB allocations.
