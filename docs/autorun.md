# What Carafe relies on in Autorun

Facts about [Autorun](https://github.com/autorunhq/autorun) that Carafe builds on, read from its code at commit
`636913c` (2026-10-03). Paths are relative to the Autorun repository (`external/autorun`) unless they start with
`runtime/` or `app/`. `hw/` stands for `horizon-wine/source/`. Anything checked only by reading code says so;
anything not checked at all is marked "not verified".

## Licenses and composition

- Autorun is LGPL-2.1-or-later (`LICENSE:1-10`). It used to be called Wine-NX.
- Components (`README.md:200-215`): FEX — MIT, Box64 — MIT, DXVK — zlib, VKD3D-Proton — LGPL,
  LSFG-VK — GPL-3.0+, Atmosphère and Horizon-OC — GPL-2.0.
- The only submodule is `horizon-dlls` ([autorun-horizon-dlls](https://github.com/autorunhq/autorun-horizon-dlls)),
  about 1.2 GB. Its licenses are in `LICENSES/` and `NOTICE.md`.
- Box64 and llvm-mingw are downloaded during the build: Box64 in `horizon-wine/tools/bootstrap-box64-core.sh:5,13`,
  llvm-mingw in `build.sh:45-63`.
- FEX is wired in by the patch `horizon-wine/fex/horizon.patch`. Its sources, `FEX-Emu/FEX@395b132f` (FEX-2609), are
  fetched by `horizon-dlls/tools/build-fex.sh:12` (`horizon-dlls@5d6eccb`).

## Build

- `build.sh` → Docker image `ghcr.io/autorunhq/switch-dev:2026.10.01` (`horizon-wine/switch-dev.txt`) → CMake
  (`horizon-wine/CMakeLists.txt`) → `tools/package-autorun.py` → `autorun-NNN.zip`. The runtime itself is
  `wine-nx-runtime.nro`.
- Autorun already builds an NSO and an NPDM, but for its loader, not for the runtime: `CMakeLists.txt:611-740` builds
  `wine-nx-hbl`, a modified nx-hbloader. Its NPDM (`horizon-wine/hbl/hbl.json`) is an Application with all SVCs
  0x00–0xBF, `force_debug_prod`, `address_space_type` 1, thread priorities 28–63 and cores 0–3.
- What the build downloads (read from the scripts and the GitHub API, 2026-10-04):
  - `horizon-dlls` at `5d6eccb` (`build.sh:40-44`, `.gitmodules`) — about 545 MB to transfer, 1.17 GB on disk;
  - llvm-mingw `ucrt-ubuntu-22.04-aarch64` 20260505 (`build.sh:46-62`) — 74 MB;
  - Box64 at `2f130fa`, depth 1 (`tools/bootstrap-box64-core.sh:5-15`);
  - FFmpeg at `3a0867c`, depth 1, for WMA (`tools/bootstrap-wma.sh:5-15`);
  - Atmosphère 1.11.2 and Horizon-OC 2.5.1 (`build-boot-bundle.sh:24-77`) — only for the quick SD install of
    Autorun; without them `build-amd64-components.sh:27-36` builds with an empty `boot_bundle`.
- The runtime `.nro` holds only the glue code for FEX (`CMakeLists.txt:832-836`); FEX itself comes as DLLs from
  `horizon-dlls`.
- In `horizon-dlls`, `switch/wine/drive_c/windows` takes 903 MB; a compressed copy in `compressed/` takes 195 MB.
- Autorun's release archive has no DLLs: the runtime downloads them by a manifest (`build.sh:14-15`,
  `package-autorun.py:269`). Carafe puts them into each NSP instead.
- Wine's `configure` (needs bison) runs on the host in Autorun's script; the `switch-dev` image has bison 3.8.2,
  flex, git, ninja, cmake and python3, so the whole build can run in the container. Carafe does exactly that
  (`runtime/Makefile`).
- Windows Defender may flag some 32-bit Wine DLLs from `horizon-dlls` as malware (machine-learning heuristics);
  the files match the `horizon-dlls` manifest. Exclude the build folders and the library folder from scanning.

## JIT and executable memory

- Box64 and FEX get their code memory through `svcCreateCodeMemory` + `svcControlCodeMemory` (owner RW, slave RX):
  `dlls/ntdll/unix/horizon.c:18827-18885`, `source/fex_jit.c:59`. The fallback is `jitCreate`, but only with
  `JitType_CodeMemory` (`source/wow64_box64_dynarec.c:235-245`).
- Atmosphère allows at most 10 CodeMemory objects for the whole system (`documentation/technical.md:176-178`).
- Memory that Wine sees:
  - RW → RX through `svcMapProcessCodeMemory`, `svcUnmapProcessCodeMemory` and `svcSetProcessMemoryPermission` on
    `envGetOwnProcessHandle()`;
  - sections through `svcMapProcessMemory` and `svcUnmapProcessMemory`;
  - without hints for SVC 0x73/0x77/0x78 or without the process handle these calls return `ENOSYS`
    (`horizon.c:17384-17392`, `18627-18636`).
- **The runtime depends on a handle to its own process.** libnx gets it only from a loader
  (`EntryType_ProcessHandle`); an NSO started directly is left with `INVALID_HANDLE`
  (libnx `nx/source/runtime/env.c:21,37-40,86-87`).
- How hbl gets the handle (`hbl/source/main.c:145-190`): `svcCreateSession` makes a server/client pair; a thread
  with a heap stack and priority 0x20 waits in `svcReplyAndReceive`; the main thread sends a request with one
  copy-handle `CUR_PROCESS_HANDLE`; the kernel turns the pseudo-handle into a real one, and the thread takes it
  from `copy_handles[0]`.
- The trick works only because hbl **closes `sm` first**: `__appInit` in `main.c:616-645` opens sm, setsys and fs,
  then calls `smExit()`. The Application group has a session limit of 1
  (`stratosphere/pm/source/impl/pm_spec.cpp:73-78`, Atmosphère `c8b7316`), and the kernel charges both
  `svcCreateSession` (`libmesosphere/source/svc/kern_svc_session.cpp:34`) and port connections such as `sm:`
  (`kern_k_client_port.cpp:68`) against it. The standard libnx `__appInit` keeps `sm` open until exit
  (`nx/source/runtime/init.c:104-149`). Checked on the console: with `sm` open, `svcCreateSession` returns
  `0x10801` (2001-0132, `LimitReached`).
- What hbl passes to the runtime (`main.c:524-576`): only standard libnx `ConfigEntry` records (the homebrew ABI) —
  MainThreadHandle, ProcessHandle, AppletType (SystemApplication + ApplicationOverride), OverrideHeap, Argv,
  NextLoadPath, LastLoadResult, SyscallAvailableHint and Hint2 (all SVCs, minus 0x4B/0x4C when CodeMemory is not
  available), RandomSeed, UserIdStorage, HosVersion, EndOfList. Autorun adds no records of its own; the only special
  thing in hbl is a load address above `WINE_NX_NATIVE_BASE` for the low-window mode (`main.c:479-491`). So the
  runtime can be started by another loader that speaks the same protocol, without changing Autorun. Carafe's loader
  in `runtime/loader/` does that.
- Same-process CodeMemory (`main.c:206-233`): on the stock kernel 5.0.0+ the creator of a CodeMemory cannot call
  `svcControlCodeMemory` itself (0xD401). Mesosphère allows it; hbl checks `detectMesosphere()`, and on other kernels
  probes with a deliberately wrong operation — 0xF001 means a patched kernel.
- How hbl makes code executable (`main.c:479-515`): `svcMapProcessCodeMemory` from the heap to an address from
  `virtmemFindCodeMemory`, then `svcSetProcessMemoryPermission` per segment.
- SVCs the NPDM needs: 0x4B, 0x4C, 0x73, 0x74, 0x75, 0x77, 0x78; the profiler also needs 0x32 and 0x33
  (`source/thread_profile.c:567`).
- Applet type: hbl runs as SystemApplication with ApplicationOverride (`main.c:527`); the runtime sets
  `__nx_applet_type = AppletType_Application` (`runtime.c:71`).

## Hard-coded paths

- The root is fixed at build time: `dlls/ntdll/unix/horizon_runtime_paths.h:4-11` (`sdmc:/switch/wine`,
  `Z:\switch\wine`, `drive_c`, `system32`, `syswow64`, the `.nro` path) and `hw/runtime.c:80-95` (WINE_ROOT,
  RUNTIME_DIR, logs, config). These are `#define`s without `#ifndef`; there is no environment variable, argument or
  setting for them.
- Other fixed paths: the session `.shm` (`horizon.c:5766`), the trace log (`horizon.c:13875`),
  `source/forwarder_launch.h:7-8`, `source/lsfg.cpp:21-22`, `source/launcher_list.h:25`, `source/main.cpp:36-37`.
- Logs (`/logs`), the config (`/config/settings.json`) and the user profile (`drive_c/users/steamuser`) are all
  written under the same root.

### How Wine sees the console's files

- Drive letters map to libnx devices (`dlls/ntdll/unix/file.c:4058-4095`): `C:` → `WINE_NX_RUNTIME_DRIVE_C`,
  `Z:` → `sdmc:`, `D:`–`H:` → `ums0:`–`ums4:` (USB) when mounted. The same table is repeated in
  `dlls/ntdll/loader.c:1458-1459` and `:3713-3714`.
- ntdll already opens `\??\unix\romfs:/…` and `\??\unix\sdmc:/…` (`file.c:3999-4020`).
- System DLLs are looked up by the constants `WINE_NX_RUNTIME_SYSTEM32`/`SYSWOW64` (`dlls/ntdll/unix/loader.c:1251`,
  `dlls/ntdll/loader.c:1513,3900`); `lib/wine` and `share/wine` hang off `WINE_NX_RUNTIME_ROOT`
  (`unix/loader.c:437-464`); the registry lives in `HORIZON_REGISTRY_DIR` (`horizon_registry_server.h:11`).
- The runtime writes into the same root: logs, config, the `steamuser` profile, `args.txt`, `target.txt`, the session
  `.shm`, the program catalogue (`runtime.c:3462`), and it recreates the `drive_c` tree on every start
  (`runtime.c:3520-3548`).
- A launch target is accepted only from `sdmc:` or `ums` (`runtime.c:1561`); anything else becomes `C:\name`.
- Before a game starts, the runtime checks the DLLs and, if they are missing, sends the user to the settings to
  download them (`runtime.c:3685-3699`, `horizon_dlls_ready`). The check (`horizon_dlls.c:1610-1655`) reads the
  manifest and checks only 11 base DLLs on disk by size: `ntdll`, `wow64`, `wow64win`, `win32u`, `winebox64`,
  `apisetschema`, `kernel32`, `kernelbase` in system32 and `ntdll`, `kernel32`, `kernelbase` in syswow64.
- Writes to save data need `fsdevCommitDevice`, otherwise they are lost; Autorun does that only in its installer
  (`autorun_install.c:174`, `setup_boot.c:181`).

### The `sdmc:` hook point

- The runtime already replaces a device: `wine_nx_sd_cache_install()`, the first line of `main` (`runtime.c:3503`),
  finds `sdmc:` in `devoptab_list` and installs its own copy with a read cache and delayed writes
  (`sd_cache.c:729-758`).
- Only `swap_store.c:253` (`fsFsOpenFile`) goes to the SD card around the device, and only with
  `WINE_NX_SWAP_POC`, which is off by default (`CMakeLists.txt:860-869`).
- A runtime started as an `.nro` cannot mount the process RomFS from the NSP by itself: `romfsInit` for an `.nro`
  reads the `.nro` file. It takes an explicit `fsOpenDataStorageByCurrentProcess` + `romfsMountFromStorage` under
  a device name of its own.
- This is where Carafe hooks in: `runtime/overlay/carafe_overlay.c` installs its own device under `sdmc:`.
  `/switch/wine/…` is read from save data or, if absent there, from the RomFS; writes go to save data; the rest of
  `sdmc:` goes to the real card. It is compiled into the runtime from outside, through
  `runtime/overlay/carafe-overlay.cmake`, without editing Autorun's files.

What the runtime keeps open for writing, which matters for committing save data:

- for the whole session: `logs/autorun_runtime.log` (`runtime.c:3549`), `logs/stdout.txt`, `logs/stderr.txt`
  (`runtime.c:1847-1849`), the session file `wine-nx-session-<pid>.shm` — 2 MB, `O_RDWR` until exit
  (`horizon.c:752`, `5757-5795`);
- briefly: the registry — written to `.tmp`, closed, renamed (`horizon_registry_server.h:176-195`).

Committing save data with a file open for writing is `WriteModeFileNotClosed` (2002-6457) in Atmosphère
(`libvapours/.../fs_results.hpp:565`), and the SDK client aborts on it
(`libstratosphere/source/fs/fsa/fs_filesystem_accessor.cpp:265-269`). What the FS service answers to libnx's
`fsFsCommit` with open files is not verified. Carafe's overlay therefore commits only when nothing in save data is
open for writing (`commitIfQuiet` in `runtime/overlay/carafe_overlay.c`).

newlib and libnx details the overlay has to respect:

- `FindDevice`/`GetDeviceOpTab` match the name only up to the colon; without a colon they return the default device
  (devkitPro newlib, `libgloss/libsysbase/iosupport.c:38-61,100-108`).
- libnx `stat` opens the file for reading to get its size (`nx/source/runtime/devices/fs_dev.c:1103-1160`), and
  Horizon's FS refuses to open a file that is already open for writing, so `stat` on such a file fails. Existence
  is checked with `fsFsGetEntryType` instead.
- libnx functions take their device from `r->deviceData`, which newlib sets before the call; a wrapper that forwards
  to another device must set that device's `deviceData` itself.

## Address space

- Tested by Autorun's authors on the console (`documentation/technical.md:18-38`): 7-Zip, Notepad, OpenTTD 15.3
  (x86, OpenGL, up to 60 fps), Quake III (Quake3e), WarCraft III, NFSU2, Halo CE, Left 4 Dead 2 (to the menu),
  D3D9 and Vulkan tests, Win64 tests.
- `technical.md:85-93`, `295-298`: games with a fixed base (NFSU2) need a 32-bit address space ("32-bit, no alias"
  forwarder, 2 GB memory limit); AMD64 programs need a 39-bit forwarder.
- The code at `636913c` disagrees with `technical.md` on the 32-bit path. The launcher runs a game only in a 39-bit
  process (`hw/launcher.c:1978-1982`, refusal at `2010-2015`), and the forwarder sets only type 3
  (`forwarder.c:702`). A game without relocations and a base below 4 GB (`hw/launcher_catalog.c:110-117`) needs
  a patched Mesosphere and loader (`horizon-wine/mesosphere/README.txt`) that gives the window 0x200000–4 GB only to
  Title ID `0x0548EABB35576000` (`mesosphere-1.12.0-low-window.patch`, `HasAutorunLowWindow`). The patch changes the
  kernel's layout at process creation, so an NSP cannot replace it. Without it the runtime logs
  `[LOWVA] stock layout; fixed-low Win32 titles require the Atmosphere low-address patch` (`low_window.c:137`).
- The unpatched runtime does not work in a 32-bit process (NPDM `address_space_type` 2), checked on NFSU: the libnx
  heap takes 2 GB of 4, the guest gets fragments (`early guest reservations: 1405 MB, largest 310 MB`), and after
  `[INIT] virtual memory ready` the runtime exits from `virtual_alloc_first_thread_data`
  (`dlls/ntdll/unix/virtual.c:5002-5031`).
- Carafe solves this with a 32-bit process and patches `0006` and `0007` in `runtime/patches/autorun/`; the
  investigation is in [research/fixed-base/REPORT.md](research/fixed-base/REPORT.md). The builder picks the NPDM
  address space from the `.exe` header (`app/crates/carafe-core/src/pe.rs`, `npdm.rs`).
- Where the kernel puts the heap is decided at process creation (ASLR). Rarely it lands inside the low 4 GB; then
  early guest reservations shrink to about 600 MB and the start fails. Ordinary Autorun likely has the same; not
  verified.
- Before the first program the runtime runs `C:\windows\autorun-setup.exe` (codecs, DirectShow), puts the program
  into `run-next.txt` and restarts; the mark is `registry/components-1.done` (`technical.md`, around line 290).
- Steam versions of games: the documentation says nothing about running them without the Steam client; not verified.

## Case of letters in paths

- Wine on Horizon treats the file system as case-insensitive: `is_case_sensitive = FALSE`
  (`dlls/ntdll/unix/file.c:296`). After a missed exact `stat`, `find_file_in_dir` (`file.c:3222-3255`) scans the
  folder ignoring case only if the FS is case-sensitive or the name looks like 8.3; otherwise it reports "not found".
  That holds on the SD card (FAT/exFAT), but not in RomFS.
- Checked on the console: a game in RomFS failed with `import_dll … USER32.dll … not found` (`c0000135`) because the
  file was `user32.dll`. Carafe's overlay therefore looks names up ignoring case, both in RomFS and in save data.
- libnx `romfs_dirnext` returns `.` and `..` first (`nx/source/runtime/devices/romfs_dev.c:852-885`); newlib's
  `readdir` hides them. Code that walks RomFS directly must skip them, or it loops.

## The on-screen keyboard layer

- Every output path takes the keyboard picture from `osk.c` (`osk.h:58-67`):
  - OpenGL — `nx_osk_draw` before `eglSwapBuffers` (`dlls/win32u/winnx_opengl.c:145-237, 250-260`): checks
    `wine_nx_osk_visible`, then `wine_nx_osk_frame` and `wine_nx_osk_copy`, and blits into the window's back buffer;
  - Vulkan — a copy into the swapchain image before present (`dlls/win32u/vulkan.c:3929-3950`);
  - the compositor — the top layer (`hw/compositor.c:442-480`); input polling asks for a redraw when
    `wine_nx_osk_generation` changes (`runtime.c:893-898`).
- Input depends on `wine_nx_osk_visible`: buttons go to the keyboard (`runtime.c:876-891`), XInput is zeroed
  (`xinput_unix.c:64`).
- The input polling thread (`dlls/win32u/winnx_drv.c:90-140`) starts with the game's first window; before that the
  compositor redraws only on its own events.
- Wine calls `eglSwapBuffers` through `funcs->p_eglSwapBuffers`, so the linker's `--wrap` does not catch it.
- `__nx_applet_auto_notifyrunning` in libnx is a weak symbol; `appletNotifyRunning` dismisses the system boot splash
  (`applet.h:1508`).
- Carafe's loading screen (`runtime/overlay/carafe_loading.c`) draws through this layer by wrapping the keyboard
  picture. Keeping it on screen until the game's first frame takes patches `0008`–`0011`; see
  [research/loading-screen/REPORT.md](research/loading-screen/REPORT.md).

## Game exit

- When a program ends, Autorun restarts itself into its launcher: `[EXIT] starting this program again for the
  launcher` (`envSetNextLoad` without arguments). In an NSP with a game that is an extra screen, so Carafe's loader
  closes the application instead (`runtime/loader/source/app_exit.c`).
- The exit path: the program calls `NtTerminateProcess` → `wine_nx_leave_process` (`runtime.c:3039-3055`) →
  `longjmp` into `main` → `return_to_launcher` (`runtime.c:3200-3370`), which stops its threads, returns memory to
  the driver, sets `envSetNextLoad` and returns 0 from `main`. Then come newlib's `exit()`, `__libnx_exit` and the
  return to the loader.
- Destructors (`__attribute__((destructor))`, `.fini_array`) are never called in the runtime: `exit` calls
  `__call_exitprocs`, which walks only the `atexit` list, and `__libc_fini_array` is not in the ELF at all. The
  runtime uses `atexit`, so the strong `__call_exitprocs` from `__call_atexit.o` is linked, and `--gc-sections`
  drops the weak one together with its reference to `__libc_fini_array`. `__libnx_exit` itself calls only
  `__appExit` and `__nx_exit`. Carafe's overlay therefore shuts down through `atexit`
  (`runtime/overlay/carafe_overlay.c`); a thread left alive after exit keeps its stack mapped, and the loader cannot
  unload the `.nro`.

## Start-up hangs

OpenTTD x86 under Autorun sometimes hung at start-up. The investigation found several separate causes:

- **Thread connections got each other's pipes.** Clients hand file descriptors to the in-process server through one
  queue shared by the whole process (`horizon_server_send_fd`, `horizon.c:16632-16635`), and the server takes the
  first one (`horizon_server_take_client_fd`, `horizon.c:4479-4482`) for new threads and for `init_thread`
  (`horizon.c:11537`, `6609-6610`, `6671-6672`). When the main thread creates many threads at once, descriptors get
  mixed up and a connection serves the wrong thread. FEX then compares the TID from `NtQueryInformationThread`
  (the connection's) with `GetCurrentThreadId()` (the TEB's), decides the thread is foreign and suspends it
  (`horizon-wine/fex/horizon.patch:1888-1896`). On Horizon a self-suspend takes effect only at the next wait —
  the `select` after the thread terminated itself — and nobody resumes it (`horizon.c:15738-15763`). Patch `0002`
  tags each descriptor with its owner.
- **Endless retries in the alias region.** In a 39-bit space the process has a 64 GB alias region that Wine does
  not know about. Code cannot be mapped there, `map_code_memory_range` turns the kernel's `0xd401` into `EEXIST`
  (`horizon.c:17441-17470`), and Wine's free-area search (`virtual.c:2009-2030`) steps down 64 KB at a time — about
  a million probes. Patch `0003` reads the alias region with `svcGetInfo` and refuses it up front.
- **Lost wakeups.** Threads waited in Wine's `RtlWaitOnAddress` (`dlls/ntdll/sync.c:877-918`) for an alert that was
  never sent: every earlier wait got its wakeup, the last one did not. Turning on FEX TSO
  (`FEX_TSOENABLED = 1`) did not help and made start-up slower, so memory ordering is not the cause. The root cause
  is still unknown. Patch `0005` waits in 250 ms slices instead of forever, which got OpenTTD to the menu in 10 runs
  out of 10.
- Diagnostics: patch `0001` records thread and server events and `0004` counts alerts and waits; together with
  `runtime/overlay/carafe_diag.c` they print `[CARAFE-DIAG]` reports when start-up stalls. They stay in release
  builds.
- Start-up can still be slow — up to about a minute — partly because threads contend for newlib's single malloc
  lock under FEX. FEX keeps translated code only in memory (`horizon-wine/fex/code_storage.h`), so there is no cache
  that makes the second start faster.

## Controls

- By default the controller acts as a mouse and keyboard (`README.md:120-140`): the right stick or touch moves the
  cursor, A/B are mouse buttons, the D-pad and left stick are arrows, + is Esc, X/Y are Space/F, L/R are Tab/Shift;
  holding + and − together for a second closes the game. Games with gamepad support see an Xbox 360 controller
  through XInput/DirectInput.
- Autorun sets layouts in its launcher. An NSP has no launcher, so Carafe writes the layout as ready files into the
  RomFS (`app/crates/carafe-core/src/autorun_files.rs`), in the format below.
- Mouse motion from the stick reaches Wine only when the game's main thread processes messages
  (`wine_nx_drv_ProcessEvents`, `dlls/win32u/winnx_drv.c:776-850`). Patch `0012` flushes it so the cursor follows
  the stick.

## Settings files

Every file is lines of `key = value` (`hw/launcher_settings.h:29-42`).

### Input layers (`hw/input_profile.c`)

- Order, each layer overriding the previous (`input_profile_global` `:146-156`, `input_profile_program` `:158-173`):
  `<root>/keys.txt` → `<root>/config/keys.txt` → `<exe without .exe>.keys.txt` → the program settings file
  `<exe without .exe>.wine-nx.txt`.
- Defaults (`input_profile_defaults` `:54-71`): controller mode, dead zones 5.
- Keys (`input_profile_apply` `:111-144`):
  - `input-mode` — 1 controller, 2 keyboard and mouse;
  - `left-deadzone`, `right-deadzone` — 0…25;
  - `keyboard-auto` — 0 or 1;
  - `pad.<BUTTON>` — a Switch button → an Xbox button, target 1…24 (labels in `input_pad_labels` `:24-31`);
  - `key.<BUTTON>` — a Switch button → a key (VK code) or a mouse action (`INPUT_MOUSE_*`);
  - `<BUTTON>` without a prefix — the old `key.*` format, still read.
- Switch buttons are the 24 names of `input_sources` (`:7-21`): `A B X Y L R ZL ZR PLUS MINUS STICKL STICKR UP DOWN
  LEFT RIGHT LUP LDOWN LLEFT LRIGHT RUP RDOWN RLEFT RRIGHT`.
- A value is a code or a chord joined with `+`, up to 4 codes (`INPUT_CHORD_MAX`, `input_profile.h:8`); `none` or
  `0` means unbound (`input_binding_parse` `:73-99`). Codes are written in hex, `0x..` (`input_binding_save`
  `:188-198`).
- Autorun maps Switch buttons to Xbox buttons by position, not by label: Switch B (bottom) → Xbox A (bottom),
  Switch A (right) → Xbox B, likewise Y → X and X → Y (`dlls/xinput1_3/nx_pad.h:82-85`). Whether `pad.*` keys
  apply on top of this table or instead of it is not verified.

### Program settings (`hw/launcher_settings.h`)

- Path: `<exe without .exe>.wine-nx.txt` next to the `.exe` (`launcher_settings_path` `:348-356`); for games on
  USB — `program-settings/<FNV-1a of the path>.wine-nx.txt` in the runtime folder (`:364-383`).
- Keys (`launcher_settings_read` `:395-468`, `launcher_settings_write` `:471-510`):
  - `title`, `hidden` — for the launcher's library;
  - `verbose`, `profile` — debugging, `1`/`0`;
  - `sync` — `horizon` or not;
  - `cpu` — `box64`, otherwise FEX;
  - `four-cores` — on by default, `0` turns it off;
  - `windows` — `framebuffer` or `compositor`;
  - `d3d` (formerly `d3d9`) — `wine`, `dxvk`, `dxvk+vkd3d`;
  - `dxvk-source` — `sarek`, `gplasync`, otherwise the official one; `dxvk-version`, `vkd3d-version`;
  - `dxvk-hud` — `0`, `fps`, `api,fps,frametimes`, or the full set (`launcher_hud_values` `:219-220`);
  - `frame-limit` — 0, 30, 40, 45, 60, 75, 90, 120 (`:215-217`); `vsync` — on by default;
  - `lsfg`, `lsfg-performance`, `lsfg-flow` (`0.125`, `0.25`, `0.5`) — LSFG-VK frame generation, needs the user's
    own copy of Lossless Scaling (`README.md:25-26`, `:114-117`);
  - `upscaling` (formerly `upscale`) — `off`, `fsr`, `integer`; `upscaling-sharpness` (formerly `sharpness`) —
    0…100 % in steps of 20 (`:221-225`).

### What each setting does

Read from the code; not checked on the console unless stated.

- **The shared file** `sdmc:/switch/wine/config/settings.json` (`hw/runtime.c:93-94, 3611-3646`): `verbose-log`,
  `profiler`, `windows-through-opengl` (true = compositor), `keyboard-on-text-focus` (true) and service keys. The
  program file overrides it (`hw/runtime.c:3805-3842`). `sync`, `cpu`, `four-cores`, `d3d` and the graphics keys have
  no shared level: without the key the code default applies (`:3811-3829`). Quick setup does not touch per-game
  settings (`hw/launcher_setup.c:16-19, 105-260`).
- **`sync = horizon`**: `select`, events, mutexes and semaphores are handled in the calling thread, without the pipe
  to the server thread (`dlls/ntdll/unix/server.c:367-416`, `horizon.c:15892-15950`). The default is standard
  (`hw/runtime.c:3820`). With OpenTTD x86 it raised successful starts from 2 in 5 to 4 in 5, before the hang fixes.
- **`cpu`**: FEX (`libwow64fex`/`libarm64ecfex`) or Box64 (`hw/runtime.c:2323, 2414`); Box64 is a second translator
  for 32-bit programs only (`README.md:18-20`).
- **`four-cores`**: graphics threads (`dxvk-cs`, `wined3d_cs`, `vkd3d_queue`) and the compositor go to core 3 at
  priority 63; other threads do not get core 3 (`hw/thread_profile.c:113-152, 234-252, 320-323`). The NPDM must allow
  core 3 and priority 63; Carafe's loader does (`runtime/loader/carafe-loader.json`). Checked on the console:
  `[CORES] graphics workers may use core 3 at priority 63`.
- **`windows`**: there is no `auto`; a missing key takes the shared `windows-through-opengl`
  (`hw/launcher_settings.h:406-411`). Framebuffer copies window pixels to the 1280×720 screen
  (`hw/runtime.c:530-579`), "for when the compositor misbehaves" (`hw/launcher.c:2443-2445`). Whether it removes the
  window frame is not verified.
- **`d3d`**: the line `d3d=dxvk` reads as DXVK+VKD3D (`hw/launcher_settings.h:414-416`), so plain DXVK is the
  absence of the key. `wine` is wined3d on OpenGL, and `DXVK_*` are cleared (`hw/runtime.c:1706-1741, 1823`).
- **`dxvk-source`**: only the official DXVK ships with the runtime; Sarek and GPLAsync are downloaded by the launcher
  (`hw/launcher_graphics.c:179`); Sarek is incompatible with VKD3D (`hw/launcher_settings.h:429-436`).
- **`dxvk-hud`**: sets `DXVK_HUD` (`hw/runtime.c:1826-1829`); an enabled HUD adds
  `dxvk.enableDescriptorBuffer = False` (`hw/launcher_settings.h:237-242`).
- **`frame-limit`, `vsync`**: a pause before presenting a frame in winevulkan (`dlls/win32u/vulkan.c:82-112, 4078`)
  plus `maxFrameRate`/`syncInterval` in `dxvk.conf` (`hw/launcher_settings.h:234-251`); they override the game's
  choice. Vulkan/DXVK/VKD3D only; OpenGL and wined3d follow the game's interval
  (`dlls/win32u/winnx_opengl.c:116`).
- **`upscaling`**: only when the Vulkan swapchain is smaller than the screen; FSR is EASU+RCAS
  (`dlls/win32u/vulkan.c:3181-3186`), integer is nearest pixel (`:262-273`). Wine's screen is always 1280×720
  (`dlls/win32u/winnx_drv.c:545-560`).
- **`lsfg`**: needs `sdmc:/switch/wine/lsfg/Lossless.dll` (`hw/lsfg.cpp:21, 220-235`). In a Carafe NSP that path goes
  to save data over the RomFS, not to the SD card, so the runtime does not see the user's file.
- **Controls**: the dead zone is circular, and the rest of the travel is stretched to the full range
  (`hw/input_profile.c:215-223`); `keyboard-auto` defaults to 1 through the shared `keyboard-on-text-focus`.
- **Profiler** (`profile = 1`, `source/thread_profile.c`): prints `[PROF]` lines with places in x86 modules and
  offsets in `wine-nx-runtime.elf`.

**DLLs next to the game (mod proxy loaders).** DLLs are searched in the game folder, then DXVK/VKD3D, then
`system32` (`hw/runtime.c:1740-1741`). For DLLs outside the system folder Wine picks native or builtin by the version
resource (`dlls/ntdll/unix/loadorder.c:425-470, 525-541`): no version resource or a non-Microsoft `CompanyName` means
native first. So an Ultimate ASI Loader `dinput8.dll` in the game folder should load without `WINEDLLOVERRIDES` or
registry edits. Not verified on the console.

**Keys the runtime reads that Carafe does not expose:**

- `dxvk-version`, `vkd3d-version` (`hw/launcher_settings.h:423-428`); a DXVK version that is not installed falls back
  to wined3d (`hw/runtime.c:1738-1739`);
- 74 DXVK options `dxvk.*`/`d3d9.*`/`d3d11.*`/`dxgi.*` (`hw/dxvk_options.c:207-470`); a `dxvk.conf` next to the game
  overrides them all (`hw/runtime.c:3872-3887`);
- 19 `FEX_*` options from `.wine-nx.txt` (`hw/runtime.c:1648-1670`, `hw/fex_options.h:46-104`);
- 18 `BOX64_DYNAREC_*` options from `<exe>.box64.txt` (`hw/wow64_box64_dynarec.c:474-498`, `hw/box64_options.h:46-101`);
- `config/settings.json` keys; the old layout format without the `key.` prefix.

## The Quick setup wizard and how to skip it

- The wizard is part of the launcher: `offer_quick_setup` (`hw/launcher.c:1779-1796`, called from `:4319`) runs it
  when the launcher opens and the view settings lack `setup-offered`.
- The launcher opens only when the runtime gets no program path: with `argv[1]` the runtime sets `autorun = 1`, takes
  the target from `argv[1]` and skips the launcher branch entirely (`runtime.c:3700-3707` versus `3708-3750`).
- Carafe's loader passes `argv` from the RomFS file `/carafe/argv` (`runtime/loader/source/main.c`,
  `setDefaultLaunch`), so neither the launcher nor the wizard appears, with no change to Autorun. Checked on the
  console: an NSP without `/carafe/argv` opens Autorun's launcher and the Quick setup wizard.

## Starting straight into a game

- `argv[1]` is the launch target, and the launcher is skipped (`runtime.c:3685-3707`).
- Other ways: `--library-game=N` through forwarders (`forwarder.c:940-945`); `run-next.txt` (`runtime.c:3673`);
  `NAME.args.txt` next to the `.exe` — its first line becomes the game's arguments (`runtime.c:1771-1777`,
  `documentation/technical.md:143-150`).
- The Windows DLLs must be present, or the launch is refused (`runtime.c:3685-3697`).

## DBI over MTP

Looked at on the console with DBI in MTP mode, through Windows Explorer (`Shell.Application`) and PnP. The DBI
version was not recorded.

- The device: WPD class, name "Switch", manufacturer Nintendo, `USB\VID_057E&PID_201D\<serial>`.
- Storages: `1: SD Card`, `5: SD Card install`, `6: NAND install`, `7: Saves`, `8: Album`. The numbers have gaps, so
  Carafe matches the name without the number (`app/crates/carafe-core/src/dbi.rs`).
- Logs of Carafe games on the card: `1: SD Card\switch\carafe\<Title ID>\` — `carafe-overlay*.log`, the runtime's
  `logs` folder and the session file `wine-nx-session-*.shm`.
- Installing into `5: SD Card install`: DBI installs the game while the file is being transferred and shows the total
  time at the end. Once, after 100 %, closing the file (`Commit`) returned a device error although the game was
  installed; the cause is not known.

## Loading-screen images and font

- The runtime links libpng and turbojpeg from devkitPro portlibs (`horizon-wine/CMakeLists.txt:888-889`, `:987`) and
  decodes the launcher's PNG and JPEG with them (`hw/launcher_image.c:15-41`). The overlay is compiled into the same
  executable, so it can use both without new libraries.
- Autorun keeps its own images as plain files (`horizon-wine/assets/logo.png`) and embeds them at build time:
  `CMakeLists.txt:721-737` runs `tools/make-embed.py`, and the bytes land in `forwarder_embed.c` in the build folder.
- The on-screen keyboard font is the console's shared font: `wine_nx_osk_font` (`hw/runtime.c:782-793`) takes
  `PlSharedFontType_Standard` through `plGetSharedFontByType` and draws with SDL_ttf (`osk.c:405-440`). It has
  Cyrillic, since the Switch UI is available in Russian.
- Carafe's loader NPDM allows all services (`runtime/loader/carafe-loader.json`, `service_access: ["*"]`), so the
  overlay can open `set` to learn the console language.

## Not investigated

- How the runtime maps drive C: and how the Wine prefix is laid out.
- Where exactly DLLs are downloaded by the manifest and how to turn that off.
- How Box64 parameters are set (environment, rc files, per-game settings).
- Why OpenGL hangs in 64-bit (AMD64) programs under FEX; Autorun's authors tested OpenTTD x86.
