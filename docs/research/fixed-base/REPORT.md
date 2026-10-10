# x86 games with a fixed base in a Carafe NSP

How Carafe runs 32-bit Windows games whose `.exe` has no relocations and must load at its fixed base (usually
`0x400000`), such as Need for Speed Underground (NFSU). The analysis was done on 2026-10-07 from the sources
listed below and from experiments on a host; nothing in it relies on changing the system's kernel.

**Outcome.** The recommended solution went into Carafe:

- P1 is [`runtime/patches/autorun/0006-virtual-32bit-address-space.patch`](../../../runtime/patches/autorun/0006-virtual-32bit-address-space.patch);
- P2 is [`runtime/patches/autorun/0007-low-window-32bit-guest-range.patch`](../../../runtime/patches/autorun/0007-low-window-32bit-guest-range.patch);
- P3 was replaced by a step in the loader, before the runtime starts:
  [`runtime/loader/source/fixed_image.c`](../../../runtime/loader/source/fixed_image.c) checks the image range,
  reserves it, and restarts the program through `RestartProgram` when the kernel has already taken it (at most
  4 times);
- the builder picks the NPDM type from the `.exe` header: `required_address_space` in
  `app/crates/carafe-core/src/pe.rs`, `set_address_space` in `app/crates/carafe-core/src/npdm.rs`.

On the console NFSU started 6 times out of 6: `[USD] … at 0xffff0000`, the image at `0x400000`, no restarts
needed, the game playable with a controller; the longest session lasted 3.8 hours.

**Corrections after checking against autorun@636913c:**

- confirmed: `exit(1)` when 0x7ffe0000 is not available (`dlls/ntdll/unix/virtual.c:258, 5014-5022`); thread data
  only above 4 GB (`:5024-5025`, `:5175`, `limit_low` in the `map_view` signature, `:2775-2777`);
- confirmed: the rest of the page relocation is still in place — the `wine_nx_user_shared_data` export
  (`dlls/ntdll/ntdll.spec:1778`), its reads in `kernel32/kernel_main.c:137`, `kernelbase/sync.c:57`,
  `ntdll/loader.c:4213`, `ntdll/unix/loader.c:1787`, redirected reads in `ntdll/unix/horizon.c:16677, 16789`;
  only the step in `virtual.c` was lost;
- confirmed through the GitHub API: `b231b265` (2026-09-14) "ntdll: Find KUSER_SHARED_DATA where Horizon lets
  it be mapped", `d542cf7b` (2026-09-24) "wine: upgrade Horizon runtime to Wine 11.18";
- **wrong:** "`system_resource_size` must be 0". The check in `ldr_process_creation.cpp`
  (`GetCreateProcessParameter`) is the bitwise `flags & AddressSpace64Bit39`, and type 2 passes it; a type 2 NSP
  with 16 MB started and reached `[INIT] virtual memory ready`. Carafe keeps 16 MB.

Source versions used for the analysis (every reference below is `repository@commit:file:lines`):

| Short | Repository and commit |
|---|---|
| `AR` | autorunhq/autorun@636913ca46a3638018b3ea56995d6852db9f0165 (lines before the patch unless stated otherwise) |
| `HD` | autorunhq/autorun-horizon-dlls@5d6eccb82c266f037dd4305037c986b6303cd59a (submodule `AR:horizon-dlls`) |
| `AMS` | Atmosphere-NX/Atmosphere@36cc9a9f490da37dd1d5f958ee9a256be132cc1d (master on 2026-10-06) |
| `NX` | switchbrew/libnx@feebd026 |
| `FEX` | FEX-Emu/FEX@395b132f346b1a45def246d10c52245edba1ef02 (FEX-2609) |
| `B64` | ptitSeb/box64@2f130fab |

The FEX sources come from `HD:tools/build-fex.sh:9-30`: it downloads `https://github.com/FEX-Emu/FEX.git` at
`revision=395b132f…` (line 12) and applies `AR:horizon-wine/fex/horizon.patch` (line 13).

---

## 1. Summary in five points

1. **There is a working solution, and only one:** a 32-bit process without an alias region (NPDM
   `address_space_type = 2`) plus a three-part fix. The other five directions provably do not work: by
   Mesosphere's code, nothing below `0x8000000` can be mapped by any SVC in the 36- and 39-bit layouts.
2. **Why it did not work before.** In a 32-bit process execution reaches `exit(1)` in
   `virtual_alloc_first_thread_data` for two reasons, both introduced by the move to Wine 11.18 (`d542cf7b`,
   2026-09-24) — the day after Autorun stopped shipping 32-bit forwarders. First, the relocation of the
   `KUSER_SHARED_DATA` page was lost. Second, thread data is now allocated only **above** 4 GB, and a 32-bit
   process has nothing there. Both `map_view` calls fail, and the second would fail for every new thread.
3. **Confidence.** The chain up to `entry` is checked against the code with high confidence. The changed files
   compile for aarch64; the host test `check-low-window.sh` passes with the patch and fails without it. The 32-bit
   profile (type 2) itself had already run on hardware: NFSU2 reached gameplay and Halo ran with DXVK, both on
   Box64 (`AR:documentation/technical.md:27-28`). FEX in a 32-bit process had most likely never run on hardware:
   FEX arrived on the same day the 32-bit forwarders were retired.
4. **Conditions.** The whole process gets 2 GB of memory. The guest gets between ≈383 MB and ≈1.4 GB below 2 GB;
   the exact amount is random on every launch. In about 1.2–2 % of launches the kernel puts one of its objects on
   the image range before our code runs; then the process is restarted.
5. **Console check:** 1 launch of NFSU and 1 control launch of a game with relocations; if FEX fails, 1 more with
   `cpu = box64`. The expected log lines are listed in section 4.

---

## 2. Directions

| # | Direction | Verdict | Key reference | Reason |
|---|---|---|---|---|
| 1 | 32-bit process (NPDM 0/2) + runtime fix | **Works under conditions** (type 2; type 0 is worse) | `AMS:libraries/libmesosphere/source/kern_k_page_table_base.cpp:189-209`, `AR:dlls/ntdll/unix/virtual.c:5013-5030` | The small map `0x200000–0x40000000` is available to `MapProcessCodeMemory`. The runtime already handles almost all of the 4 GB layout; three changes are missing (section 3). Conditions: 2 GB of memory, a random amount of guest VA, about 1.5 % restarts |
| 2 | Shift every guest address (A → A+base) | Does not work as a minimal patch | `FEX` has no guest base mechanism (grep for `membase/guestbase` in `FEXCore` finds only code cache relocations, `FEXCore/Source/Interface/Core/JIT/JITClass.h:606-609`); `AR:dlls/wow64` — 121 uses of `ULongToPtr`/`PtrToUlong`, `wow64win` — 151, `winevulkan` — 3372, `opengl32` — 2027 | Every memory operation of the JIT would change (loads, stores, atomics, x87/SSE save/restore, REP MOVS/STOS, code fetch in the Frontend, SMC tracking), as would every WoW64 32→64 transition where native code dereferences a guest pointer, plus the TEB32/PEB32 structures written by unix-ntdll. The change cannot be kept local |
| 3 | Shift only the image (trap or translation) | Does not work; completeness cannot be proven | `AMS:…/svc/kern_svc_synchronization.cpp:85-88`, `AMS:…/kern_k_page_table_base.cpp:3743-3797` | In a 39-bit process `0x400000` cannot be mapped at all, so any access there faults. But when the kernel gets such a pointer in an SVC or IPC, it does not raise an exception in the process — it returns `ResultInvalidPointer`/`InvalidCurrentMemory`, and nothing can intercept that. A trap would also fire on every access to a global variable: thousands of exceptions per second |
| 4 | Rebuild relocations on the PC | Does not work | — | A 32-bit value in the image range can be a pointer or a number: `5000000 = 0x004C4B40` lies inside `0x400000–0x795000`. There are addresses like "table base minus offset" (`[ecx*4+0x3FFFFC]`), encrypted or packed code (DRM), code checksums. Completeness cannot be proven in principle |
| 5 | Horizon means without a kernel patch (SVC, NPDM, a second process) | **Impossible** for 36/39 bits (proven) | `AMS:…/kern_k_page_table_base.cpp:165-181, 439-577`; `AMS:libraries/libvapours/include/vapours/svc/svc_memory_map.hpp:101,109` | Every state a user process can map is limited to a region that starts at `0x8000000` (section 2.1) |
| 6a | The real 0x7ffe0000 page taken from the heap | Possible but invasive | `AMS:…/kern_k_page_table_base.cpp:519-534` | In type 2 the 0x7ffe0000 page almost always lies in the heap region. It could be taken from the heap, but the runtime's libnx heap would then have to handle two ranges. Not needed for the solution |
| 6b | `hbl_config`/override in Atmosphère | Ruled out by the requirements | `AMS:stratosphere/loader/source/ldr_meta.cpp:242-273` | Needs files on the SD card outside the NSP; in effect the same type 2 |
| 6c | Turn ASLR off from the NPDM | Impossible | `AMS:stratosphere/loader/source/ldr_process_creation.cpp:439-442` | `EnableAslr` depends only on `ldr_flags`, set by pm, not by the NPDM |

### 2.1. Proof of point 5: nothing can be mapped below `0x8000000` in 36/39 bits

1. **Where the regions start.** For 39 bits the code and alias-code region is `AddressMap39 = [128 MB, 512 GB)`
   (`svc_memory_map.hpp:109-111`; `kern_k_page_table_base.cpp:169-172`). Heap, alias, stack and kernel-map are
   laid out inside `[m_code_region_start, …)` (lines 177-180, 238-275, 336-349). For 36 bits:
   `m_code_region_start = SmallMap36Start = 0 + 128 MB` (`svc_memory_map.hpp:101`;
   `kern_k_page_table_base.cpp:193-208`), stack and kernel-map coincide with the code region (lines 197-200), and
   `after_process_code_start = 2 GB` (line 207).
2. **Where mapping is allowed.** `GetRegionAddress` (lines 439-474) and `CanContain` (513-577) assign a region to
   each state:
   - Normal → heap;
   - Stack → stack;
   - Static/ThreadLocal → kernel-map;
   - Io, Shared, AliasCode(Data), Transfered, SharedCode, GeneratedCode, CodeOut, Insecure → alias-code;
   - Code → the code region.

   Only Free and Kernel return `m_address_space_start = 0` (`kern_k_page_table.cpp:145`), and no user SVC creates
   mappings in those states.
3. **Every SVC checks this:**
   - `svcMapProcessCodeMemory`: `CanContain(AliasCode)`, `kern_k_page_table_base.cpp:1004`;
   - `svcMapMemory`: `CanContain(Stack)`, `svc/kern_svc_memory.cpp:94,118`;
   - `svcMapProcessMemory`: `CanContain(SharedCode)`, `svc/kern_svc_process_memory.cpp:82,124`;
   - `svcMapProcessCodeMemory` itself in the SVC layer: lines 151,178;
   - `svcMapSharedMemory`: `svc/kern_svc_shared_memory.cpp:57,85`;
   - `svcMapTransferMemory`: `svc/kern_svc_transfer_memory.cpp:50,71`;
   - `svcControlCodeMemory`: `svc/kern_svc_code_memory.cpp:89-125`;
   - `svcMapInsecurePhysicalMemory`: `svc/kern_svc_insecure_memory.cpp:33,48`;
   - `svcMapPhysicalMemory`: `IsInAliasRegion`, `svc/kern_svc_physical_memory.cpp:63,85`;
   - `svcSetHeapSize` — the heap region only.

   `svcSetProcessMemoryPermission` only changes permissions on an existing mapping.
4. **Neither ASLR nor the NPDM changes this.** The NPDM type goes straight into the flags
   (`ldr_process_creation.cpp:413-431`), and none of the 36/39-bit types starts below 128 MB.
5. **A second process does not help.** The guest's code runs in the JIT inside our process. Another process would
   help only if all of Wine moved there, and that is direction 1.
6. **Experiment.** A line-by-line C port of `InitializeForProcess` (lines 136-388), 200,000 random layouts per
   type, `gcc -O2`:

```
type0 32-bit           lowest mappable 0x000200000  0x7ffe0000: heap 49.940% alias 50.060% mappable 0.000%
type1 36-bit           lowest mappable 0x008000000  0x7ffe0000: heap 0.000% alias 0.000% mappable 100.000%
type2 32-bit no alias  lowest mappable 0x000200000  0x7ffe0000: heap 99.790% alias 0.000% mappable 0.210%
type3 39-bit           lowest mappable 0x008000000  0x7ffe0000: heap 0.252% alias 0.234% mappable 99.514%
```

Two conclusions follow:

- addresses below `0x8000000` exist only in types 0 and 2;
- in types 0 and 2 the address `0x7ffe0000` is practically never mappable (type 2: 512 of 513 heap positions).
  So `KUSER_SHARED_DATA` must always be moved in a 32-bit process; it is not bad luck.

### 2.2. Why Autorun gave up the 32-bit path (from its history)

- `5845078a` (2026-09-13) — notes on running NFSU2 (`documentation/horizon-x86-memory.md` in that commit). The
  sequence:
  1. In 39 bits — `STATUS_CONFLICTING_ADDRESSES`, as in our log.
  2. Switch to `32_bit` and relocation of USD (`b231b265`, 2026-09-13).
  3. Out of memory at 1 GB.
  4. `32_bit_without_alias` gives 2 GB, and the game reaches its intro.
- `e48307a1` (2026-09-18): type 0 → type 2, because Halo hit the alias gigabyte (`map_code failed`,
  `VK_ERROR_OUT_OF_HOST_MEMORY`).
- `a64a58a8` (2026-09-23): "In a 32-bit address space the game shares the low 4 GB with Wine, Box64's code and
  DXVK's memory, and a large one runs out and closes" — the launcher started sending games to 39 bits.
- `3c851da6` (2026-09-23): the low-window kernel patch. `8fc05045` (2026-09-23): "retire 32-bit forwarders"; the
  forwarder always writes type 3 (`forwarder.c:702`).
- `d542cf7b` (2026-09-24): the move to Wine 11.18. It removed the USD relocation and made `limit_4g` the lower
  bound for thread data. Check: `git show d542cf7b^:dlls/ntdll/unix/virtual.c | grep limit_4g` — before the
  upgrade there are no such `map_view` calls. Nobody ran the 32-bit path after that.
- The documentation stayed as it was: `AR:documentation/technical.md:85-88` ("32-bit, no alias" for NFSU2) and
  `:295-296` (2 GB).

In short: the path was given up for **capacity** (large games did not fit in 2 GB of memory and 4 GB of VA), and
the Wine upgrade broke it after that. The rest of the code for the 4 GB layout is still in the runtime:

- `virtual.c:4425-4446` — the 5/8 window for 4 GB;
- `virtual.c:5454-5463` — the stack in a 32-bit process;
- `horizon.c:17028-17140` — the heap region as a system view;
- `horizon.c:18960-19010` — JIT arenas in the window;
- `fex_jit.c:124-129` and `fex/cache_policy.h:9-28` — FEX in "narrow" mode;
- `win32u/vulkan.c:136-145,398-406` — Vulkan in a 32-bit process;
- `horizon_virtmem.c:54-80` — stacks in the window.

---

## 3. Recommended solution

### 3.1. What changes

| Part | Where | Required? | Purpose |
|---|---|---|---|
| NPDM | Carafe's builder | yes | `address_space_type = 2`, only for LOW images |
| P1 | `dlls/ntdll/unix/virtual.c` — patch `0006` | yes | Move USD; thread data below 4 GB when nothing is above |
| P2 | `horizon-wine/source/low_window.c` — patch `0007` | recommended | From libnx's first placement on, keeps it from putting objects into the guest's part of the small map |
| P3 | the loader, `runtime/loader/source/fixed_image.c` | recommended | If the kernel or the loader took the image range before the runtime, restart the process (at most 4 times in a row) |

The loader's heap is computed from `TotalMemorySize` (`AR:horizon-wine/hbl/source/main.c:116-142`) and becomes
≈1.86 GB in type 2 on its own; mapping the NRO through `virtmemFindCodeMemory` works in 32 bits (Autorun's
forwarders worked that way).

The draft P3 sat in `horizon-wine/source/runtime.c` and relaunched through `appletRequestLaunchApplication(0)`.
Carafe moved it into its own loader instead: the range is checked and reserved before the runtime is loaded, and
the restart uses the system's `RestartProgram`. A test build with a forced taken range restarted 4 times on the
console and then started the game.

Checks of the draft patch:

- `git apply --check` against `AR` passes;
- compiling with clang `--target=aarch64-linux-gnu` and the flags from `horizon-wine/CMakeLists.txt:46-62`: no new
  errors or warnings in `virtual.c` and `runtime.c` (the set of errors caused by stubbed generated headers is the
  same before and after);
- `low_window.c` compiles cleanly with `-Wall -Wextra`;
- `CC=gcc sh horizon-wine/check-low-window.sh` passes all 4 tests with the patch; the new test fails without P2
  (`Assertion 'reserved_count == 1 && reservation.base == 0x200000 …' failed`).

### 3.2. The patches

- **P1** (`0006`) adds `thread_data_limit_low`: with a host address space above 4 GB it returns `limit_4g` as
  before; otherwise it returns 0 and asks for `MEM_TOP_DOWN`, as `virtual_alloc_thread_stack` does. Both
  `virtual_alloc_first_thread_data` and `virtual_alloc_thread_data` use it. When the fixed mapping of
  `0x7ffe0000` fails in an address space of 4 GB or less, a page below 4 GB is mapped top-down,
  `user_shared_data` points to it, and the runtime log gets
  `[USD] 0x7ffe0000 unavailable (…); shared user data at …`.
- **P2** (`0007`) adds `reserve_32bit_guest_range`, called from `wine_nx_low_window_reserve` on the first profile
  check.
- **P3** is `fixedImageClaim` in the loader.

### 3.3. How it is enabled only for the games that need it

- **Builder.** It takes the same sign from the `.exe` as `AR:horizon-wine/source/launcher_catalog.c:110-117`: no
  relocations (or `RELOCS_STRIPPED` set), no `DYNAMIC_BASE`, `ImageBase < 4 GB`. Such an image is
  `LAUNCHER_ADDRESS_LOW`, and only for it the NPDM META gets `flags` (offset 0x0C) = `(flags & ~0x0E) | (2 << 1)`.

  The field layout is in `AR:horizon-wine/source/forwarder.c:158-175`, the mask and shift at `:686-687`. In
  `hbl.json` this is `"address_space_type": 2`. Everything else (all SVCs, `force_debug_prod`, Title ID) stays.
  `is_64_bit` stays `true`; the kernel allows this combination: `AMS:…/svc/kern_svc_process.cpp:142-158` (only
  36/39/42 check `is_64_bit`).
- **`system_resource_size`.** The draft set it to 0, citing
  `AMS:stratosphere/loader/source/ldr_process_creation.cpp:523-528` (`R_UNLESS(… AddressSpace64Bit39 …,
  ResultInvalidMeta)`); with 0 the process uses the shared `ApplicationSystemResource`
  (`AMS:…/kern_k_process.cpp:288`), as Autorun's 32-bit forwarders did before `217eeac4`. The check turned out to
  be bitwise (see the corrections above), and Carafe keeps 16 MB.
- **Runtime.** Each change works only in an address space of 4 GB or less:
  - P1 — `host_addr_space_limit <= limit_4g`; with a larger space `thread_data_limit_low` returns the old
    `limit_4g` and leaves `alloc_type` alone, so the calls are byte for byte the same;
  - P2 — `aslr_end <= 4 GB`; the test checks that the 36- and 39-bit profiles get no reservation;
  - P3 — the loader acts only in a 32-bit address space and for an i386 image without relocations.

  Games with relocations are built as before, with type 3, and none of the changed lines runs for them.

### 3.4. Proof chain: from process creation to the first accesses to the image

Notation: H — start of the heap region; "window" — `[0x18140000, 0x40000000)`.

| # | Step | Code | Why it passes |
|---|---|---|---|
| 0 | ldr reads the NPDM | `AMS:ldr_process_creation.cpp:413-431, 440-442, 523-528` | Type 2 → `AddressSpace32BitNoReserved`, ASLR on |
| 1 | Process code | `…:592-645` | `aslr_start = SmallMap32Start (0x200000)`, a shift that is a multiple of 2 MB within the small map |
| 2 | `CreateProcess` check | `AMS:…/svc/kern_svc_process.cpp:142-158` | 32-bit types do not check `is_64_bit` |
| 3 | Layout | `AMS:…/kern_k_page_table_base.cpp:189-209, 238-349`; `kern_k_address_space_info.cpp:90-105` | Code, stack and kernel-map = `[0x200000, 0x40000000)`. The 2 GB heap lies in `[1 GB, 4 GB)`, H is random ∈ {1 GB + k·2 MB, k = 0…512} (`RegionAlignment = 2 MB`, `kern_k_page_table_base.hpp:114`). No alias region |
| 4 | Memory limit | `AMS:…/kern_k_process.cpp:229` | `m_max_process_memory` = heap region size = 2 GB |
| 5 | Main thread stack and TLS — before our code | `kern_k_process.cpp:953-959`; `FindFreeArea` `kern_k_page_table_base.cpp:1252-1293` | Placed at random in the small map. The chance to overlap `0x400000–0x795000` is ≈1.19 % (code, stack and TLS together, by simulation). Detected and handled by P3 |
| 6 | Loader: heap and NRO | `AR:hbl/source/main.c:116-142, 479-492` | Heap ≈1.86 GB starting at H (the trial log shows `heap region 005b200000+2097152KB`). The NRO goes at random outside the heap; chance to hit the image ≈0.27 % (P3) |
| 7 | The runtime's libnx, first placement | `--wrap` in `AR:horizon-wine/CMakeLists.txt:880-883`; P2 | `wine_nx_low_window_reserve` reserves `[0x200000, 0x18140000)`. `_memregionFindRandom` skips reservations (`NX:nx/source/kernel/virtmem.c` `_memregionIsReserved`). So the runtime's stacks (`thread.c:131`), shmem and tmem no longer land on the image before `virtual_init` |
| 8 | `[LOWVA]` log line | `AR:low_window.c:134-138` | Reports `stock layout`, `low_window_available = 0` |
| 9 | `virtual_init` | `AR:virtual.c:4563-4600, 4391-4467` | The heap below 4 GB becomes a `VPROT_SYSTEM` view. Window = min(768 MB, 5/8 of the region) → starts at `0x18140000` (matches the trial log). Everything free below 4 GB outside the window is reserved for the guest |
| 10 | USD | `AR:virtual.c:5013-5022` + P1; `map_fixed_area` 2726 | In 99.79 % of launches `0x7ffe0000` lies inside the system heap view: `find_view_range` → `STATUS_CONFLICTING_ADDRESSES` (`c0000018`), no side effects. P1 takes a page below 4 GB, top-down (in the reserved guest area above H+2 GB), and updates `user_shared_data`. If H = `0x80000000` (0.21 %), mapping at the original address succeeds |
| 11 | First thread data | `AR:virtual.c:5024-5025` + P1 | Before: `limit_low = 4 GB` with `end = host limit = 4 GB` → `map_free_area` finds nothing → `STATUS_NO_MEMORY` → `exit(1)`. Now `limit_low = 0`, top-down |
| 12 | Mapping `Speed.exe` | `map_image_view` `virtual.c:3943-4022`; `map_fixed_area` → `anon_mmap_fixed` → `horizon_mmap_fixed` (`horizon.c:19752-19812`) → `svcMapProcessCodeMemory` | The target `0x400000` ∈ alias-code `[0x200000, 4 GB)` and outside the heap — `CanContain(AliasCode)` (`kern_k_page_table_base.cpp:513-577`) lets it through. The range is free by steps 5-7 |
| 13 | Image check | `AR:runtime.c:2498-2508` | The image sits at `0x400000` → no "cannot be moved" line |
| 14 | TEB and PEB | `AR:virtual.c:4982-4988, 5057-5062` | `set_large_address_space`: `user_space_wow_limit = 2 GB − 1` (or 4 GB − 1 with LAA); the TEB block is not above that limit. `ImageCharacteristics` come from `runtime.c:2476` |
| 15 | USD for every consumer | unix: a variable (`virtual.c:258` and on, the clock `virtual.c:5573-5590`); i386 ntdll: `AR:dlls/ntdll/unix/loader.c:1787-1789`; ARM64 ntdll: `AR:dlls/ntdll/loader.c:4213-4234`; kernelbase and kernel32: `AR:dlls/kernelbase/sync.c:50-57`, `kernel32/kernel_main.c` | The `wine_nx_user_shared_data` export is in the `HD` binaries: the string is in `syswow64/ntdll.dll`, `kernel32.dll`, `kernelbase.dll`, `system32/ntdll.dll`. A disassembly of **all** i386 modules in `HD:switch/wine/drive_c/windows/syswow64` found one real memory access at a fixed address: `kernelbase!GetLargePageMinimum`, `mov eax,[0x7ffe0244]` (`dlls/kernelbase/memory.c:43,154`). The other matches are bit masks in `user32`/`oleaut32`. FEX and Box64 do not touch `0x7ffe…`/`SharedUserData` |
| 16 | Direct reads of 0x7ffe0xxx | `AR:dlls/ntdll/unix/horizon.c:16677-16712`, called first in `__libnx_exception_handler` (`:16789`); for FEX threads — `AR:horizon-wine/source/fex_context.S:12-32`, then `horizon_resume_exception` (`horizon.c:294-309`) | The load is emulated from the moved page; a write stays an access violation (on Windows the page is read-only too). Coverage checked by experiment (section 5) |
| 17 | WoW64 start and `entry` | `AR:runtime.c:4111` → `runtime_start_wow64`; `runtime.c:2278-2279, 2331` | Every "≤ 0xffffffff" check holds, since a 32-bit process has nothing above. `entry` = `0x400000 + 0x270cb5 = 0x670cb5` |
| 18 | First accesses to the image | FEX: guest and host addresses are the same | A read of `[0x4xxxxx]` from JIT code is an ordinary load from mapped memory. SMC protection and `VirtualProtect` go through the same mapping layer as in the 39-bit profile |

**Why nothing breaks further on:**

- **Threads.**
  - Thread data: P1, `virtual_alloc_thread_data`.
  - The 64-bit stack at `thread.c:1255` (`limit_4g`) is already handled in `virtual.c:5454-5463`.
  - The 32-bit stack is limited by `user_space_wow_limit` (`thread.c:1263-1266`).
  - TEB: `virtual.c:5120-5121`.
  - Kernel TLS pages: `__wrap_svcCreateThread` creates the thread again if the page fell into a reservation
    (`horizon.c:19702-19731`); until the program starts, `hold_thread_local_pages` holds them (`runtime.c:3095`,
    called at `:4077`; removal from reservations — `virtual.c:4371-4389`).
  - Native stacks go into the window (`horizon_virtmem.c:54-80`).
- **Exceptions and SEH.** The path does not depend on the layout (step 16). The `fs:[0]` chain lies in TEB32
  below 2 GB.
- **win32u callbacks.** They use the 64-bit stack (step 14 and threads above) and the guest's 32-bit stack;
  nothing above 4 GB is needed.
- **DXVK and Vulkan.** In a 32-bit process `nx_driver_maps_preferred()` picks the driver's own mappings
  (`AR:dlls/win32u/vulkan.c:136-145, 398-406`). They lie below 4 GB, and since everything else is reserved for the
  guest, in the window, that is, below 1 GB. So programs without LAA can see them.
- **FEX: JIT and code cache.**
  - `WINE_NX_FEX_WIDE_HOST` is not set (`runtime.c:1636-1645`).
  - Narrow mode: L1 — 64K entries, L2 — 4 MB, cache limit — 256 MB (`fex/cache_policy.h:9-28`); the L2 index is
    hashed for any address (`fex/horizon.patch:445-470`).
  - Arenas of at most 64 MB with 64 KB alignment (`fex_jit.c:124-129`) go into the window, or into a Wine view
    below 4 GB when the window is full (`horizon.c:18960-19010`).
  - FEX allocations without an address are not forced above 4 GB (`AllocateNativeMemory` in
    `fex/horizon.patch:1747`, condition `!WideHost()`).
- **The game's allocations.** For programs without LAA `default_zero_bits` limits allocations to 2 GB
  (`AR:dlls/wow64/wow64_private.h:94-97`; `virtual.c:6544-6547`). Room below 2 GB: `[0x200000, 0x18140000)` =
  383 MB minus kernel objects and Wine's DLLs, plus `[1 GB, H)` — uniformly from 0 to 1024 MB. With probability
  1/8 that extra piece is under 128 MB.
- **Limits.** The whole process has 2 GB of memory: runtime, Mesa, JIT, Wine and the game together.

### 3.5. Why this is not a hack

- **It closes the whole class.** Any PE32 without relocations whose image fits in `[0x200000, 0x18140000)` works:
  the usual `0x400000` base and DLLs with bases like `0x10000000`. They take the same path as in 39 bits (Wine puts
  the image at its base). No heuristics.
- **The USD relocation is exact.** Wine works through a pointer, and direct reads are emulated per instruction.
- **P3 is not a timed wait.** It is a deterministic "range taken" check with a bounded number of attempts. It
  repeats because Mesosphere places the stack, TLS and code at random by design, and a process cannot move them.

---

## 4. Console check (fewest launches)

What to build: Carafe with the patches. An NFSU NSP with NPDM type 2. A control NSP — a game with relocations
(type 3, unchanged).

**Launch 1: NFSU, FEX by default.** Lines are expected in this order:

| Expected line | If it is missing |
|---|---|
| `[carafe-loader] start: fixed image 0x400000-0x795000, attempt 0, …` and `… is free after 0 restarts; reserved` | The loader did not see a 32-bit address space: check the NPDM type (byte `0x0C` in META) |
| `[BUILD] … (address space 32 bits)` | The NPDM is not type 2 |
| `[LOWVA] stock layout; …` | — |
| `[MAP] start-up: heap region 00XXXXXXXX+2097152KB` | Not 2 GB → not type 2 |
| `[VA] native stack region 0x200000-0x40000000; window for native mappings 0x18140000-0x40000000; stacks compact` | A different window start means the formula differs from P2 |
| `[VA] early guest reservations: N MB, largest M MB` | `largest` must be ≥ 300 MB. N is the guest memory in this launch |
| `[INIT] virtual memory ready` | — |
| `[USD] 0x7ffe0000 unavailable (c0000018); shared user data at 0x…` (address > `0x80000000`, status `00000000`) | No line and no crash — H = `0x80000000` (normal, 0.2 %). A crash with `failed to map the shared user data` — P1 is not applied |
| `[INIT] server process initialized` | `failed to allocate initial thread data` — P1 (thread data) is not applied |
| `[IMAGE] base=0x400000 size=0x395000 preferred=0x400000 …` without "cannot be moved" | If the loader logged `… is taken: … type=…` and `restarting the program (1 of 4)` instead, that is expected (≈1.5 %): the next log starts with a new process. `type` shows who took the range (Stack — a kernel stack, ThreadLocal — TLS, Code — the loader or the NRO) |
| `[WOW64] guest ntdll=… init block status=00000000` | — |
| `[WOW64] loader ready: TEB32=0x… stack=… entry=00670cb5` | TEB32 and the stack must be < `0x80000000` |
| No `failed to create main module` | If present — look at `horizon-trace.log` (verbose) for `[IV]`/`[HMAP] fixed replacement failed …` |
| `[NXVK] Vulkan memory uses native driver mappings` (with `d3d=dxvk`) | `low host imports` means Vulkan treats the space as 39-bit |
| `[PROGRESS]` every 10 s, `heap_free_mb` > 100 | `code_mb` growing towards 256 MB means the JIT window is running out |
| `[USD] N reads of 0x7ffe0000 served …` (only with direct reads) | No line — the game does not read USD directly (expected) |

**Launch 2: the control game with relocations (type 3).** The log must match the old one in `[BUILD] … 39 bits`
and `[VA] …`. There must be no `[USD]` lines and no loader restart.

**Launch 3 (only if FEX failed after `entry`):** NFSU with `cpu = box64` in `Speed.wine-nx.txt`. Box64 had already
run in the 32-bit profile on hardware (NFSU2).

Useful for analysis: `logs/autorun_runtime.log`, `logs/Speed.log`, and with verbose on — `horizon-trace.log`. The
system crash report, if the process ended on its own.

---

## 5. Experiments on a host

| Experiment | How | Result |
|---|---|---|
| `InitializeForProcess` layout for types 0–3 | C port of the kernel code | The table in section 2.1 |
| Risk of a collision with kernel and loader objects | Python simulation, 200,000 trials | NFSU: code 0.38 %, stack 0.45 %, TLS 0.36 %, at least one — **1.19 %**. NFSU2 — 1.68 %, GH3 (`0x400000–0x28f1000`) — 10.7 % |
| Direct accesses to 0x7ffe0xxx in the `HD` i386 DLLs | capstone, linear pass over executable sections | Only `kernelbase.dll 0x7b0383e5 mov eax,[0x7ffe0244]` |
| The `horizon_read_redirect.h` read emulator | Autorun's own test ported to Linux under `qemu-aarch64 -cpu max`, plus direct calls | Every GPR form (LDR/LDUR/LDRS*/LDP/LDPSW, pre/post-index, register offset, SP) matches the CPU. SIMD forms are correct when called directly (q, d, s — OK). In the signal test under qemu-user they did not match: the handler emulates them, but on return qemu-user does not apply the SIMD registers changed in the signal frame (inference, not checked). **Not emulated:** `LDAR`, `LDAPR`, `LDAPUR`, `LDXR`, `LD1` |
| Compiling the patch | clang `--target=aarch64-linux-gnu` + libnx headers, flags from CMakeLists | No new errors or warnings |
| Autorun's host test | `CC=gcc sh horizon-wine/check-low-window.sh` | Passes with the patch; the new test fails on the old code |
| Build in the official image | `ghcr.io/autorunhq/switch-dev:2026.10.01` | Not done during the analysis (no network access to the registry); the patches were later built in Carafe's container |

---

## 6. Devil's advocate

| Objection | Answer |
|---|---|
| "The game reads 0x7ffe0000 itself" | The exception handler serves the reads (step 16). It does not understand acquire and exclusive loads (section 5); FEX emits them only with `FEX_TSOENABLED=1`, and TSO is off by default (`AR:horizon-wine/source/fex_options.h:47-48`). If a game needs TSO and also reads USD directly, LDAR/LDAPR/LDAPUR must be added to `horizon_read_redirect.h` — not part of this patch. Frequent reads show up in the `[USD]` lines |
| "The kernel put a stack on the image" | P3 restarts the process, at most 4 times in a row. The chance that every attempt fails is ≈ 0.015⁵ ≈ 10⁻⁹. Without P3 ≈1.5 % of launches would fail with a clear log line |
| "Not enough VA below 2 GB" | In the worst case (H = 1 GB) ≈383 MB remain, minus DLLs. NFSU2, a heavier game, reached gameplay on this profile. Whether that is enough for NFSU was not proven in advance; the amount is in the `[VA] early guest reservations` line |
| "2 GB of memory is not enough" | A kernel limit; it cannot be worked around. For a 2003 game the margin is large; for big games from 2008 on this path does not fit (that is why Autorun gave it up in `a64a58a8`) |
| "Without `system_resource_size` the kernel runs out of memory blocks" | Moot: Carafe keeps 16 MB. With 0, the shared application slab has 20,000 blocks (`AMS:…/kern_kernel.hpp:63`), and Autorun's 32-bit forwarders worked with it (NFSU2, Halo). A shortage would show as `[HMAP] … errno` or `rc=0x…` with `OutOfResource` |
| "FEX in narrow mode was never tested on hardware" | The code is written for that mode (cache_policy, fex_jit), and `tests/fex_lookup_cache.cpp` covers both modes. The fallback is `cpu = box64` |
| "JIT takes the 638 MB window" | FEX arenas that do not fit in the window go to a Wine view below 4 GB (`horizon.c:18996-18999`), that is, above 2 GB, out of reach of programs without LAA |
| "P2 gets in the runtime's way before `virtual_init`" | Native code keeps the window (638 MB) and the whole large region outside the heap. The same approach already works in the low-window profile (`low_window.c:26-28`) |
| "The 39-bit path breaks" | Every change is cut off by the ≤ 4 GB condition (section 3.3); the test checks that 36/39 bits get no reservation |
| "Carafe's own code (alias, the `sdmc:` overlay)" | The trial log shows `[CARAFE] alias region 0x0-0x0`, after which execution reached `[INIT] virtual memory ready`, so it copes with an empty alias region |
| "The system refuses to restart the program" | The loader then logs `the system did not restart the program (0x…); starting anyway` and goes on — no worse than without P3 |

---

## 7. What was unproven and how it turned out

| # | Claim | Status |
|---|---|---|
| 1 | After `entry` NFSU runs to gameplay (FEX, 32 bits) | Confirmed on the console: 6 of 6 launches, the longest 3.8 hours |
| 2 | Restarting works for a Carafe NSP | Confirmed with the loader's `RestartProgram` and a test build that forced a taken range: 4 restarts, then the game |
| 3 | The 20,000 blocks of the shared slab are enough for Wine+FEX+DXVK | Dropped: Carafe keeps `system_resource_size` at 16 MB |
| 4 | ≥ 383 MB of guest VA is enough for NFSU | Held in every console launch; the amount per launch is in `[VA] early guest reservations` |
| 5 | The Atmosphère version on a console lays out memory like `AMS@36cc9a9f` | Confirmed empirically by `[MAP] heap region …+2097152KB` and `[VA] native stack region 0x200000-0x40000000` |
| 6 | Wine 11.18 has no 32-bit regressions besides the three found | Checked by searching for `limit_4g`, `0x100000000`, `LowestStartingAddress`, `MakeWOW64AddressReqs` in `dlls/ntdll/unix`, `dlls/wow64*`, `FEX/Source/Windows`: the other places are already handled or apply only with `WideHost()`. NFSU runs; other games are not covered |
| 7 | The loader's code size (part of the 0.38 % estimate) | An estimate of 0x11000 was used |
| 8 | `Speed.exe` does not read USD with acquire loads | Not checked directly; the `[USD]` counter in the log shows direct reads |
| 9 | qemu-user is why the SIMD cases failed in the signal test | Inference, not checked; Autorun's own test (`horizon-wine/tests/horizon_read_redirect.c`) on an AArch64 Mac would settle it |
