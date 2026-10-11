# Automated testing of Carafe NSPs without a console

Whether a Carafe NSP can be tested automatically in Docker by emulating Horizon OS instead of running it on a
Switch: which emulator could run the runtime (loader → Wine with Box64/FEX JIT → Mesa nvc0/NVK over nvdrv), how to
drive it with scripted input, screenshots, logs and saves, and which test scenarios that would cover. The analysis
was done on 2026-10-10 from the Carafe tree at `ba9cbb0` plus the working tree, the pinned `external/autorun`
submodule, and the emulator sources listed at the end. Nothing was run in an emulator yet.

**Outcome.** No change went into Carafe. The verdict below decides what a spike has to prove first; the choices made
from it are in [decision 0020](../../decisions/0020-emulator-test-bench.md).

---

## 1. Summary

1. **No emulator is known to run the Carafe runtime today, and none is ruled out by the kernel side.** Eden
   (yuzu lineage) and Ryubing (Ryujinx lineage) implement every SVC the loader and runtime need, including the
   ones that are Atmosphère-only on hardware: same-process `CodeMemory` owner+slave mappings and
   `MapProcessCodeMemory` on the process's own handle. This is verified in source.
2. **The GPU path is the biggest unknown.** Both emulators emulate the GPU at the GPFIFO / Maxwell-engine level,
   so in principle they are driver-agnostic, but they implement nvdrv for NVN games. Mesa nvc0 and NVK use nvmap
   and channel ioctls in other patterns; Ryubing answers several nvmap ioctls with `NotSupported`. Not verified
   either way: Mesa's sources are not in the repository.
3. **The JIT-on-JIT cost is the second unknown.** Box64/FEX generate ARM64 code constantly; the emulator
   retranslates each block and invalidates on every `IC IVAU`. Expect it to be very slow; whether it is usable
   for a launch test of a few minutes is open.
4. **Eden is the better candidate for automation:** it has a TAS input file format, a null-window `yuzu_cmd`, a
   GDB stub and host folders for `sdmc` and saves. Ryubing has a headless mode and a GDB stub but no scripted
   input, and it still opens an SDL window.
5. **Full-system emulation of real Horizon + Atmosphère is not an option** (no GPU model, needs boot dumps).
6. **Autorun does not test on an emulator either.** It runs host unit tests and Box64/FEX tests in ARM64
   containers; Horizon-specific behaviour is tested on a console.
7. **Recommended next step:** a time-boxed spike with Eden `yuzu_cmd` in a Linux container: does the loader
   reach `[INIT] virtual memory ready`, does the loading screen draw, does Mesa get past channel set-up. If Mesa
   fails on nvdrv, emulator testing stops at "the loader and the overlay start", which is still worth a CI job.

Emulators need the user's own console dumps (firmware and system files); none of that goes into the repository
or a public CI image.

---

## 2. What the runtime needs from Horizon OS

Paths are relative to the repository; `L/` = `runtime/loader/source/`, `O/` = `runtime/overlay/`,
`hw/` = `external/autorun/horizon-wine/source/`, `horizon.c` = `external/autorun/dlls/ntdll/unix/horizon.c`.
Launcher-only Autorun code is left out: Carafe passes `argv`, so the launcher never runs.

### 2.1 Process shape

- ExeFS `main` is Carafe's loader (an NSO based on nx-hbloader); the real program is
  `switch/wine/wine-nx-runtime.nro` in RomFS, mapped by the loader from its own heap.
- NPDM from `runtime/loader/carafe-loader.json`: application, all SVCs `0x00`–`0xBF` allowed,
  `service_access: ["*"]`, full FS permissions, cores 0–3, unsigned ACID. Address space type 3 (39-bit) by
  default, type 2 (32-bit, no alias) for fixed-base games (`app/crates/carafe-core/src/npdm.rs:33-58`).
- NACP: device save 256 MB with a 64 MB journal ([nsp.md](../../nsp.md)).

### 2.2 SVCs

Loader (`L/main.c`, `L/fixed_image.c`, `L/app_exit.c`):

- `svcConnectToNamedPort("ams")` as an Atmosphère probe (`main.c:85-89`);
- `svcGetInfo` for memory sizes, kernel-version probes and ProgramId (`main.c:115-116,180,186,259`), ASLR region
  in `fixed_image.c`;
- `svcSetHeapSize` with all free memory minus `HEAP_RESERVE` (`main.c:14,112-136`);
- own process handle: `svcCreateSession` + a helper thread in `svcReplyAndReceive` + `svcSendSyncRequest` with
  `CUR_PROCESS_HANDLE` as a copy handle (`main.c:140-175`);
- `svcCreateCodeMemory` + `svcControlCodeMemory` probe for Mesosphère (`main.c:195-210`);
- `svcMapProcessCodeMemory`, `svcSetProcessMemoryPermission`, `svcUnmapProcessCodeMemory` on the own handle
  (`main.c:224,345,380`);
- `svcBreak` debugger notifications for NRO load and unload (`main.c:219,229,370,438`).

Runtime:

- JIT: `svcCreateCodeMemory` + `svcControlCodeMemory` owner RW / slave RX in the same process
  (`hw/fex_jit.c:81-91,269-275`, `horizon.c:18834-18883`; Box64 in `hw/wow64_box64_dynarec.c:235-245`);
- Wine's executable mappings via `svcMapProcessCodeMemory` / `svcSetProcessMemoryPermission`
  (`horizon.c:17422-17460,17620-17626`); shared views via `svcMapProcessMemory` (`horizon.c:19099-19105`);
- thread suspension for FEX: `svcSetThreadActivity` + `svcGetThreadContext3` (`hw/thread_profile.c:533,676`);
- the usual thread, sync, `svcWaitForAddress`, `svcQueryMemory`, `svcGetInfo` and resource-limit SVCs;
- user-mode exception handling through a wrapped `__libnx_exception_entry` (`hw/runtime.c:73-78`).

Not used: `svcMapPhysicalMemory`, debug SVCs, `svcCallSecureMonitor`, `svcCreateProcess`.

### 2.3 Services

- Loader: `sm`, `set:sys`, `fsp-srv` (own RomFS reader `L/romfs_reader.c:106-111`, device save for the 32-bit
  relaunch counter), SD card for `carafe-loader.log`; `appletOE` raw IPC in `L/app_exit.c`:
  `OpenApplicationProxy` with the own process handle, then `Exit`, or `ExecuteProgram` to restart itself.
- Runtime and overlay: `appletOE`, `hid` (pad and touch, `hw/runtime.c:855-905`), `time`, `fsp-srv` (device
  save open or create and commit, `O/carafe_overlay.c:1127,1170-1197`), `sdmc:`, `vi` / binder through
  `nwindowGetDefault`, `nvdrv`, `audout:u` (`hw/audio_unix.c`), `pl` shared font, `set`, `bsd` (a failure is only
  logged, `hw/runtime.c:3600`), `nifm`.
- `appletNotifyRunning` dismisses the boot splash (`O/carafe_loading.c:251,817`).

### 2.4 Bits that need custom firmware on hardware

- the hbloader handoff (an NSO mapping an NRO from its own heap);
- the own-process-handle trick;
- same-process `CodeMemory` (Mesosphère only on 5.0+);
- unsigned ACID with every SVC and service;
- the `ams` port probe and the `ATMOSPHR` hosversion magic (`L/main.c:85-99,431`).

No `/atmosphere` paths, `ams:*` services or mitm are used. An emulator ignores ACID signatures, and the
Atmosphère probes fail gracefully.

### 2.5 GPU path

Mesa 26 from `mesa-switch`: nvc0 (Gallium OpenGL/EGL) and NVK Vulkan over its own Horizon backend, without
`libdrm_nouveau` (`external/autorun/horizon-wine/CMakeLists.txt:898-938`). Mesa is prebuilt in the
`switch-dev` image, so its exact ioctl list is **not verified**. Inferred from libnx: `/dev/nvmap` (create,
alloc, free, param, from-id, get-id), `/dev/nvhost-as-gpu`, `/dev/nvhost-ctrl-gpu`, `/dev/nvhost-gpu` (GPFIFO
set-up and submit), `/dev/nvhost-ctrl` (syncpoints, events), plus 8 MB of transfer memory
(`hw/runtime.c:2832-2843`). NVK exchanges nvmap IDs with the compositor
(`external/autorun/dlls/win32u/winnx_vulkan.c:15-40`).

### 2.6 Memory: 96 MB, not 16 MB

The loader keeps **96 MB** outside the heap: `HEAP_RESERVE 0x6000000` in `runtime/loader/source/main.c:14`,
applied at `:125-126`. [fixed-base/REPORT.md](../fixed-base/REPORT.md) says "Carafe keeps 16 MB"; read
against its context, that figure is the NPDM `system_resource_size`, not the heap reserve. Both numbers are in
force, but the fixed-base report never mentions the 96 MB. An emulator's memory setting has to leave room for both.

---

## 3. Emulator coverage

V = verified in source, I = inferred. Ryubing line numbers come from a GitHub mirror (see Sources) and may drift
from the canonical Forgejo repository, which refused connections from this machine. Eden paths are from master.

### 3.1 Project status

- **Eden**: active, v0.2.1 (June 2026), nightlies in October 2026 (V, release page).
- **Ryubing**: active; read through a mirror synced on 2026-08-26 (V).
- **Citron**: 0.10.0 in May 2026, git restored mid-2026 (I, secondary sources).
- **sudachi**: no commits since mid-2025 (I).
- **suyu**: archived in September 2026 (I).

Citron, sudachi and suyu share yuzu's code for everything below; their sources were not read.

### 3.2 Kernel

- `MapProcessCodeMemory` / `UnmapProcessCodeMemory`, `SetProcessMemoryPermission`, `MapProcessMemory`:
  - Eden: yes, V (`src/core/hle/kernel/svc/svc_process_memory.cpp`);
  - Ryubing: yes, V (`Ryujinx.HLE/HOS/Kernel/SupervisorCall/Syscall.cs` ~1527, ~1672, ~1710).
- Same-process `CreateCodeMemory` / `ControlCodeMemory`:
  - Eden: yes, V (`svc_code_memory.cpp`);
  - Ryubing: yes, V (`Syscall.cs` ~1407, ~1448), but refused when `AllowCodeMemoryForJit` is off for the process
    (`KProcess.cs:66`, set in `ProcessLoaderHelper.cs:231,356`); whether it is on for an NSP is not verified.
- `SetHeapSize`: both yes, V. Ryubing caps it by `--dram-size` (default 4 GiB).
- `svcGetInfo` Mesosphère ids: Eden 65000 and 65001, V; Ryubing 65001 only, V (`InfoType.cs:35`).
- Own process handle through a self-IPC copy of `CUR_PROCESS_HANDLE`: I for both. The session SVCs exist; the
  pseudo-handle translation on copy was not checked.
- `SetThreadActivity` + `GetThreadContext3`: I for both (standard SVCs, present in the lists).
- Guest code invalidation on `IC IVAU`:
  - Eden: yes, V (`src/core/arm/dynarmic/arm_dynarmic_64.cpp`, per 64-byte line; `IC IALLU` clears the cache);
  - Ryubing: yes, V (`ARMeilleure/Instructions/InstEmitSystem.cs:134` → `Translation/Translator.cs:537`);
    host JIT cache entries are not freed (a TODO in `Translator.cs`).
- JIT-on-JIT speed: unknown for both (I: very slow; memory growth on Ryubing). No report of Box64, FEX or Wine
  running in any emulator was found. Ryujinx PR #2451, which added `KCodeMemory`, itself says these SVCs "alone
  don't allow for proper JIT".

### 3.3 Loading the NSP

- NSO in ExeFS: both yes, V (Ryubing `Loaders/Processes/ProcessLoader.cs:149,228`).
- HBABI: neither emulator passes hbloader config entries to an NSO, but Carafe's loader builds them itself, so
  this needs only the SVCs above (I).
- Address space type 2 (32-bit, no alias): not checked in either.

### 3.4 Filesystem and saves

- `sdmc` as a host folder:
  - Eden: yes, V (`src/core/hle/service/filesystem/fsp/fsp_srv.cpp`), path from the config ini (I);
  - Ryubing: yes, V (`AppDataManager.cs:37`, `sdcard` under the data dir or `--root-data-dir`).
- Device save create and open: Eden yes, V; Ryubing yes, I (LibHac).
- Commit semantics: Eden partial, I: writes go straight to the host folder and commit is effectively a no-op,
  so a lost commit would not show; Ryubing uses LibHac's real commit, I.

### 3.5 GPU (nvdrv)

- Eden: devices `nvmap`, `nvhost_as_gpu`, `nvhost_ctrl`, `nvhost_ctrl_gpu`, `nvhost_gpu`, `nvdisp_disp0` and the
  media engines exist, V (`src/core/hle/service/nvdrv/devices/`); per-ioctl coverage was not read.
- Ryubing, V:
  - `nvmap`: Create, FromId, Alloc, Free, Param, GetId; ioctls `0x02`, `0x06`–`0x08`, `0x0A`, `0x0C`, `0x0D`,
    `0x0F`–`0x11` return NotSupported (`NvMapDeviceFile.cs`);
  - `nvhost-as-gpu`: bind, alloc and free space, map and unmap buffer, VA regions, InitializeEx, Remap;
  - `nvhost-gpu` channel: GPFIFO alloc and submit, obj ctx, ZCULL bind, error notifier, priority, timeslice;
    several are `Logger.Stub`;
  - `nvhost-ctrl-gpu`: characteristics, TPC masks, ZCULL info; ZBC set table is stubbed;
  - `nvhost-ctrl`: syncpoints and events.
- Mesa nvc0 / NVK on top: unknown for both (I). Both translate raw Maxwell shaders and execute GPFIFO methods, so
  a well-formed command stream should work; an unsupported ioctl during set-up would stop it first.

### 3.6 Applets

- HOME / qlaunch: Eden partial, V (the QLaunch applet id is handled when firmware is installed,
  `src/core/hle/service/am/applet_manager.cpp`); Ryubing no, V (HLE library applets only, no HOME hotkey).
- `ExecuteProgram` self-restart: Eden partial, I (implemented for multi-program titles); Ryubing not checked.
- `appletOE` `Exit`: I for both; the emulator most likely just stops the program.

### 3.7 Automation

- Scripted input: Eden yes, V (TAS, see 6.1); Ryubing no, V (SDL3 input only); a uinput virtual gamepad would be
  the route (I).
- Headless: Eden `yuzu_cmd` with a null window, V (`src/yuzu_cmd/yuzu.cpp`); Ryubing `--no-gui`, V
  (`Ryujinx/Program.cs:153`), but it still creates an SDL3 window, so it needs Xvfb.
- Screenshots: both only through a hotkey (Eden I; Ryubing V, `KeyboardHotkeys.cs:6`).
- Logs to file: both yes (Ryubing V, `Options.cs:350-377`).
- GDB stub: both yes, V (Eden `src/core/debugger/gdbstub.cpp`; Ryubing `--enable-gdb-stub`, `--gdb-stub-port`,
  `--suspend-on-start`, `Options.cs:434-440`).
- Docker on Linux: both I. Vulkan via lavapipe or OpenGL via llvmpipe should start, at single-digit frame rates;
  neither project ships a Dockerfile.

---

## 4. Full-system emulation (rejected)

- **tegra_qemu** (yellows8): a QEMU fork with `-machine tegrax1` that boots bare-metal code and bootloaders. Its
  README says "Actual GPU rendering will not be supported", and it needs boot ROM and bootloader dumps from a
  console. Nothing shows it booting Horizon or Atmosphère, and without a GPU model nvservices, vi and the applet
  stack would not come up.
- **mizu** (Horizon Linux): a patched arm64 Linux kernel runs Horizon processes natively, with yuzu-derived
  services and GPU as a systemd service. It is alpha (a few homebrew titles and one game), appears
  unmaintained, and inherits yuzu's nvdrv emulation, so it adds risk without removing the GPU unknown.
- Mesosphère's own development used a barebones Tegra X1 emulator, and SunriseOS and Mirage reimplement the
  kernel; all are research-grade, and none runs userland with a GPU.

---

## 5. How Autorun tests itself

From Autorun's `documentation/technical.md` and `.gitlab-ci.yml`: it builds in Docker (`build.sh`, the
`switch-dev` image) and runs host tests: `check-runtime-console.sh` (runtime and server unit tests under
ASan/UBSan), `check-box64-execution.sh` (the Box64 interpreter and dynarec in an ARM64 container plus the Switch
build), syscall-gate, CPU DLL, AMD64 and ARM64EC tests, `tests/check-horizon-dlls.sh`. No emulator and no
desktop target that runs the Horizon Wine build is documented. The GitLab issue tracker was not checked.

The same split fits Carafe if the emulator spike fails: host unit tests for the overlay and loader logic (the
`sdmc:` devoptab, the commit thread, the loading-screen rules) and a Linux Wine + Box64 build in an ARM64
container for game-level behaviour; Horizon-specific behaviour stays on the console ([testing.md](../../testing.md)).

---

## 6. Automation building blocks

### 6.1 Eden TAS files

`src/input_common/drivers/tas_input.cpp` (V): files `script{N}-{player}.txt`, one line per input change:
`frame KEY_A;KEY_B x;y x;y`, that is, the frame number, buttons separated by `;` (`NONE` for none), and the left
and right stick as integers scaled by 32767. Recording writes `record.txt`. Playback starts through
`StartStop()` and `Record()`, which the Qt front end binds to hotkeys; `yuzu_cmd` has no documented trigger (I),
so a small patch or a config option may be needed.

### 6.2 Headless `yuzu_cmd`

`src/yuzu_cmd/yuzu.cpp` (V) offers OpenGL, Vulkan and a null window (`EmuWindow_SDL3_Null`). Recent `eden-cli`
release notes list `-n`, `-x` and `-s` options; their meaning was not read. A null window gives logs and saves
but no frames; screenshots need the Vulkan window on Xvfb.

### 6.3 GDB stub

Eden's stub supports memory and register read and write, software breakpoints and watchpoints, stepping,
`qXfer` libraries and threads, and `qRcmd` info / mappings / fastmem (V). The loader's `svcBreak` notifications
should make the NRO show up as a library (I), so breakpoints in the runtime are possible. It is a tool for
diagnosing a stall, not a test oracle.

### 6.4 Host folders

`sdmc` and NAND (saves) are plain host folders in both emulators. A test reads the logs in
`sdmc/switch/carafe/<tid>/` directly and copies or hashes the device save between runs.

### 6.5 Log milestones

The primary oracle, in launch order:

- `carafe-overlay.log`: `started`, `program 0x…, volatile root`, `romfs index:`, `installed over sdmc:…, commit
  thread running`;
- runtime log: `[INIT] virtual memory ready`;
- `[CARAFE] loading screen drawn at N s, WxH` (`runtime/overlay/carafe_loading.c:627`);
- `[CARAFE] loading screen hidden after N s: <reason>` (`carafe_loading.c:252`), the game's first frame;
- `[CARAFE] no progress for N s, reporting the threads` (`carafe_loading.c:856`), a failure marker;
- `carafe-overlay.log`: `commit <reason>: rc=…`, `shutdown`; runtime `[EXIT] …`.

### 6.6 Screenshot comparison

SSIM or a perceptual hash against a golden image with a tolerance (ImageMagick `compare -metric SSIM`), with
frames grabbed from Xvfb or by sending the screenshot hotkey with `xdotool`. OCR only for text screens.
Engine-specific test SDKs (GameDriver, AltTester, Poco) need code inside the game and do not apply.

### 6.7 Scenarios

- **Launch**: the milestones up to `loading screen hidden`, no `no progress`, within a timeout.
- **Input reaches the game**: a TAS script presses a button on a known screen; the next screenshot differs from
  the golden "before" image.
- **Save survives a restart**: the TAS script saves, the emulator stops after `commit … rc=0` and starts again;
  the save file in the host NAND folder keeps its hash and the game shows it. On Eden a missing commit would not
  show (3.4).
- **Exit to HOME and relaunch**: `[EXIT]` and the loader's `appletOE` `Exit` without a crash; relaunch is a new
  emulator process, since HOME is not reliable (3.6).
- **Loading screen**: `drawn` comes before `hidden`, and screenshots between them show the loading picture, not
  black.

### 6.8 Build settings and controls

A real game cannot report what it received, so the Autorun settings (`app/crates/carafe-core/src/settings.rs`) are
checked with probe programs: small Windows executables that log what Wine hands them and draw a known frame.

- **Settings files, no emulator.** Every value of every setting maps to the Autorun files written into the NSP;
  Rust tests cover all values cheaply.
- **Input probe.** A TAS script presses Switch buttons; the probe logs the XInput state (`InputMode::Controller`, an
  Xbox 360 pad) or the keys and mouse events (`InputMode::KeyboardMouse`, chords in `Binding::codes`), and stick
  values on both sides of `left_deadzone` / `right_deadzone`. The log is compared with the bindings in the build.
- **Graphics probes.** D3D9, D3D11, D3D12, OpenGL and Vulkan, 32 and 64 bit, draw a golden frame under the graphics
  and system settings: `Direct3d`, `DxvkSource`, `CpuEmulator`, `SyncMode`, `WindowOutput`, `Upscaling`, `DxvkHud`,
  LSFG, frame limit. All combinations run to about 250,000, so a pairwise set (every pair of values at least once,
  about 25 runs) is the practical cover; random combinations can run on top when the machine is idle.

---

## 7. Open risks for a spike

1. **Mesa ioctls on emulated nvdrv.** Whether nvc0 and NVK get through channel set-up (nvmap get-id and from-id,
   GPFIFO alloc, ZBC, the compute class) and render. Check first with the `[NXVK]` / `[NXGL]` lines and the
   emulator's stub log.
2. **Box64/FEX JIT speed.** Whether a launch reaches the first frame within minutes, and whether memory grows
   without bound in Ryubing's JIT cache.
3. **`ExecuteProgram` and `appletOE` `Exit`.** Whether the 32-bit relaunch and the exit path work, or these
   scenarios need a stub or stay console-only.
4. **TAS without hotkeys.** Whether `yuzu_cmd` can start a TAS script on boot or needs a patch.
5. **Vulkan in Docker on a Windows host.** lavapipe in the container works everywhere but is slow; a GPU through
   WSL2 (`/dev/dxg`, Mesa's dozen) is faster but not verified for Vulkan under Docker Desktop. The null window
   avoids the question for log-only tests.
6. **Own process handle and `AllowCodeMemoryForJit`.** The copy of `CUR_PROCESS_HANDLE` in both emulators and
   the JIT flag in Ryubing are small checks, but each is a hard stop for the loader.

---

## 8. Sources

- Eden: https://git.eden-emu.dev/eden-emu/eden (master, read 2026-10-10); releases
  https://git.eden-emu.dev/eden-emu/eden/releases and https://github.com/eden-emulator/Releases
- Ryubing: https://git.ryujinx.app/projects/Ryubing (not reachable from this machine); mirror
  https://github.com/Stella-sea/ryujinx-admin, branch `canary`, synced 2026-08-26; frozen original
  https://git.nadeko.net/ryujinx-mirror/Ryujinx (last commit 2024-10-01)
- Ryujinx PR #2451, KCodeMemory: https://github.com/Ryujinx/Ryujinx/pull/2451
- switchbrew: https://switchbrew.org/wiki/SVC, https://switchbrew.org/wiki/JIT_services
- tegra_qemu: https://github.com/yellows8/tegra_qemu
- mizu: https://github.com/kentjhall/mizu
- SunriseOS: https://github.com/sunriseos/SunriseOS; Mirage: https://github.com/mirage-rs/Mirage
- SciresM, kernel patching and emulation:
  https://douevenknow.us/post/178903213313/nintendo-switch-kernel-patching-and-emulation
- Autorun: https://github.com/autorunhq/autorun (`documentation/technical.md`, `.gitlab-ci.yml`)
- Game test tools: https://alttester.com/tools/, https://testguild.com/tools/gamedriver
