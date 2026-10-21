# Game speed: where CPU time goes in a translated game and how to win it back

A source review, not a measurement. It covers 32-bit Windows games, which run on the console under FEX (WoW64), with
Direct3D 9 through DXVK. FEX was read at `395b132f` (FEX-2609) with Autorun's `horizon-wine/fex/horizon.patch` at
`636913ca` and Carafe's `runtime/patches/fex`, in the patched tree `runtime/build/fex-src`. DXVK was read at
`b1a1c99a` (3.1.1). Lines marked "checked" were read a second time; the rest come from one reading pass and are not
verified. Nothing below was timed on a Cortex-A57.

## What we know

- The console has one CPU: four Cortex-A57 cores, ARMv8.0, 1020 MHz at stock and 1785 MHz in boost mode. Every change
  can be tuned and measured for this one core.
- 3D games we run are CPU-bound. Need for Speed Underground, the reference case, races at about 30 fps at 1020 MHz and
  40–60 fps at 1785 MHz with VSync on; the rate rose almost with the clock. With VSync, 30 can hide any real rate
  between 30 and 59; the rate without VSync is not measured.
- One game thread keeps one core busy: about 98 % in NFSU's race, 75–85 % in OpenTTD, with about 400 system calls per
  frame in NFSU.
- In Eden that thread spends most of its time in translated code, not in FEX itself. Eden emulates the CPU, so this is
  a hint only.

## What runs translated

- DXVK's `d3d9.dll` is an i686 PE (`runtime/dxvk/build-dxvk.sh:53`), so DXVK runs under FEX: its Direct3D 9 front end
  on the game's thread and its `dxvk-cs` worker on its own core (`d3d9_device.h:1227-1268`).
- Every DLL a game loads from `syswow64` is i386 and translated too: ntdll, kernelbase, kernel32, msvcrt, user32,
  winevulkan, dsound, dinput8.
- Native ARM64: ntdll, wow64, wow64win, `libwow64fex.dll`, and the unix side linked into `wine-nx-runtime.elf`
  (Wine's unix code, winevulkan's thunks, Mesa).
- A Vulkan call leaves the JIT through FEX's unix-call bridge (`Source/Windows/WOW64/Module.cpp:452-494`). No kernel
  transition, but a register spill and a re-entry each time.
- ARM64EC is used only for 64-bit games (`horizon-wine/source/runtime.c:3934-3937`). FEX's host thunks exist only for
  Linux and are off.

So a change to FEX speeds up the game's code, DXVK and Wine at once, in every 32-bit game.

## Measuring first

- The runtime's profiler (`profile.txt`, `thread_profile.c`) finds translated code only through Box64
  (`wine_nx_box64_pc_to_x86`, line 540). Under FEX all JIT code lands in "pe" as raw addresses, so it cannot tell a
  game from `d3d9.dll` or Wine. It needs the guest address from FEX's `CpuStateFrame`, the way
  `RestoreRIPFromHostPC` does (`Core/Core.cpp:154-188`).
- The DXVK HUD forces `dxvk.enableDescriptorBuffer = False` (`launcher_settings.h:237-242`): a run with the HUD takes a
  different path from a run without it.
- Frame rate can be measured only on the console; Carafe Lab checks that a change still works.

## Ways to win time

### Block-to-block jumps

FEX translates code in blocks and looks up the next one on every indirect jump, call through a pointer, and return.
Autorun caps the per-thread L1 lookup cache at 64K entries and leaves L2 off (`horizon-wine/fex/cache_policy.h:22-26`,
`source/fex_options.h:60-62`).

- **A clock read on every L1 miss** (checked). `UpdateDynamicL1Stats` calls `std::chrono::system_clock::now()` on
  every L2/L3 hit (`Interface/Core/LookupCache.h:237-242`), which on Windows is `NtQuerySystemTime`
  (`Source/Windows/Common/WinAPI/IO.cpp:412-416`). A cycle counter read would do. `FEX_DYNAMICL1CACHE=0` avoids it
  without a patch, at up to 1 MB per thread.
- **Most L1 slots unused** (checked). The slot is `RIP & mask` (`Interface/Core/JIT/BranchOps.cpp:209`); MSVC aligns
  functions to 16 bytes, so call targets fall into 1 of 16 slots. Hashing the address (`RIP ^ (RIP >> 4)`) in all
  four places that index L1 costs one instruction and no memory.
- **The call-return stack drifts on every system call and unix call.** Wine's i386 stubs reach the bridge with
  `call`, and the exit is a plain jump that never pops FEX's entry (`OpcodeDispatcher.cpp:62-66`,
  `WOW64/Module.cpp:452-484`). Each later return misses and goes through L1.

### x86 flags on ARMv8.0

The A57 has no FlagM. x86 and ARM keep the carry of a subtraction in opposite forms, and FEX fixes the form before
every conditional jump. Without FlagM this is `mrs nzcv; eor; msr nzcv` (checked: `OpcodeDispatcher.h:1763-1781`), and
it is emitted as a store of all four flags (checked: `RedundantFlagCalculationElimination.cpp:267-275`), so the
dead-flag pass can drop it only when every flag is dead. `test; jz` and `add; jcc` pay it every time, in any game.

- Emit `CarryInvert` on ARMv8.0 too and lower it as `mrs; eor; msr`: the pass already drops it when carry is dead
  (checked: same file, lines 291-296).
- Produce carry in the needed form to begin with for `test` and 8/16-bit logical operations
  (`OpcodeDispatcher.h:1690-1712`).
- Flag inserts (shifts, `inc`/`dec`, `bt`) as per-bit `RmifNZCV` lowered to `mrs; bfi; msr`
  (`OpcodeDispatcher.h:1719-1747`).
- The cost of `mrs`/`msr nzcv` on the A57 needs a microbenchmark on the console.

### x87

Games built with MSVC before SSE2 became the default do their float math on x87. With `X87ReducedPrecision` the x87
registers are doubles (`Core/Frontend.cpp:74-78`).

- **The x87 stack goes to memory at every block end** (checked: the pass resets per block,
  `IR/Passes/x87StackOptimizationPass.cpp:741-745`; flush at 1227-1233). The first read of a slot not pushed in the
  block moves the rest of the block to the slow memory path (`:494`). Every branch starts a new block, so branchy float
  code spills all the time. Loading such a slot on demand would keep the fast path.
- **`fnstsw ax`** syncs the whole stack to memory in the middle of a block (`OpcodeDispatcher/X87.cpp:771`), though it
  needs only TOP.
- **`fldcw`** forces the slow path and writes FPCR (checked: `OpcodeDispatcher/X87F64.cpp:48-57`). The older `_ftol`
  helper does it twice per float-to-int cast; VC7.1's `_ftol2` does not.
- The tag word is updated at every block exit, though only `fxam`, `fsave`/`fxsave`/`fstenv` and MMX read it.
- A quick check per game: `FEX_X87REDUCEDPRECISION=0` against `1`. A large drop means x87 is hot.

### Stutter

- New blocks are compiled on the thread that reached them, the game's main thread included
  (`Dispatcher.cpp:319-338`). Every FEX `malloc`/`free` and every instruction cache flush is a unix call
  (`horizon.patch:1329-1357`, `horizon-wine/source/fex_unix.c:101-129`).
- FEX-2609 has a disk cache of translated code, off and not exposed in `fex_options.h` (`Config.json.in:147-202`).
  The Horizon port loads cached code but turns off page-trap self-modifying-code detection with it
  (`horizon.patch:1534`). It would remove the stutter of code reached for the first time; it is real work to make safe.

### DXVK calls

- Recording `vkCmd*` calls in the i386 winevulkan and replaying them natively at submit would cut most bridge
  crossings on `dxvk-cs`. Weeks of work; worth it only where `dxvk-cs` turns out to be the limit.
- A native `d3d9.dll` behind a WoW64 thunk: about 16 COM interfaces and 300 methods, a native DXVK for Horizon,
  locked memory below 4 GB. Months; the largest single gain for the Direct3D side, a game's own code stays translated.

## Quick checks without code

- `cpu=box64` on the same scene: Autorun's Box64 has a unix-call gate written for NFSU2 (`wow64_box64_unix.c:17-24`).
- `FEX_DYNAMICL1CACHE=0`; `FEX_MAXINST` 2500–5000.
- `FEX_X87REDUCEDPRECISION=0` as the x87 check above.
- The same scene with and without the DXVK HUD.

Enabling L2 as it stands is not worth it: its 4 MB covers 64 guest pages of 64 KB (`LookupCache.h:481`), less than a
game's hot code. `TUNE_CPU` and LTO change only FEX's own C++, not the code it emits
(`CMakeLists.txt:21,220,546-551`).

## Expectation

Not measured. In a CPU-bound Direct3D 9 game the main thread likely splits between the game's own code, a large share
for the Direct3D 9 front end of DXVK, and the rest for jumps, Wine and system calls. FEX changes speed up all three in
every game; DXVK changes speed up only the Direct3D side. For NFSU, a steady 60 fps at 1785 MHz looks reachable;
60 fps at 1020 MHz needs 1.5–2 times faster CPU work, and only several of the ways above together could give it.

Order: the profiler that understands FEX and an A57 microbenchmark, run together with the quick checks; then the
block-jump and flag patches; then x87 and the disk cache as the profile shows; DXVK only if the profile points there.
NFSU's race and OpenTTD serve as the reference scenes.

FEX does not accept AI-written code (`CLAUDE.md` in its tree), so these patches stay in `runtime/patches/fex`.
