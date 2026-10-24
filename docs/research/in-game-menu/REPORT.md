# A settings menu over a running game

Date: 2026-10-23. What it takes to change a game's settings on the console while it runs, what Autorun offers that
Carafe does not, and how other products do it. Autorun's files are under `external/autorun/horizon-wine/source/` at
`636913ca`. The decision is [33](../../decisions/0033-in-game-settings-menu.md).

## What runs during a game

- One process: the loader (`runtime/loader/source/main.c:464-474`) maps `wine-nx-runtime.nro`, which holds Wine, the
  in-process server, FEX, DXVK and Mesa. Carafe's code enters it through `runtime/overlay/carafe-overlay.cmake:10-22`
  (`--wrap` on the on-screen keyboard layer, `main`, `pthread_create` and others) and the patches.
- Frames reach the screen three ways: Vulkan through winevulkan (`dlls/win32u/winnx_vulkan.c:50-113`,
  `dlls/win32u/vulkan.c`), OpenGL through EGL (`dlls/win32u/winnx_opengl.c`), GDI windows through Autorun's compositor
  (`compositor.c`).
- The on-screen keyboard's layer (`osk.h:1-67`) is pasted over every frame on all three paths: `vkCmdCopyBufferToImage`
  (`vulkan.c:3748-3860`), `glBlitFramebuffer` before `eglSwapBuffers` (`winnx_opengl.c` ~145-240), the top quad
  (`compositor.c:442-480`). The Vulkan and OpenGL paths copy without blending (read from the calls, not tested).
  Carafe's loading screen already takes this layer over full screen (`runtime/overlay/carafe_loading.c:740-815`).
- Threads that run beside the game: Carafe's commit thread (`runtime/overlay/carafe_overlay.c:1457-1471`), the input
  thread that polls the pad every 16 ms (`dlls/win32u/winnx_drv.c:90-141`), the compositor's presenter.

## Input

- `wine_nx_pointer_poll` (`runtime.c:841-946`) reads the pad and the touch screen, opens the keyboard on − and the right
  stick pressed (:874-879) and quits on + and − held for a second (:936-943). While the keyboard is open the pressed
  buttons are held back from the game (`wine_nx_input_swallowed`, :880-891; `xinput_unix.c:59`).
- XInput reads the pad on every `XInputGetState` and maps it through the same profile (`xinput_unix.c:37-77`).
  DirectInput uses the same state (`dlls/dinput/joystick_nx.c`).
- The profile is one global struct (`input_profile.h:23-28`) read on every poll, so mappings and dead zones can change
  while the game runs; its two readers hold different mutexes. A game may remember the device type it saw at start, so
  switching between controller and keyboard mode is left for a restart.
- No code sees a short HOME press: the application gets only applet messages, which the commit thread handles
  (`carafe_overlay.c:1434-1455`).

## Settings and when they are read

- The app writes `<exe>.keys.txt` and `<exe>.wine-nx.txt` (`app/crates/carafe-core/src/autorun_files.rs`); the runtime
  reads them once at start (`runtime.c:3805-3899`, FEX environment :1627-1673, input profile :1751-1761).
- Files under `sdmc:/switch/wine/` are read from save data first and from RomFS second, and writes go to save data
  (decision 4), so a file written on the console replaces the package's file whole.
- Could change while the game runs: the frame limit of winevulkan (`nx_frame_interval`, read every present,
  `vulkan.c:79-111`) — but DXVK's `maxFrameRate` in `dxvk.conf` is fixed at start, so the limit can only go below it.
  VSync and upscaling are taken when the swapchain is made (`vulkan.c:3129-3162`, :3185). The DXVK HUD comes from the
  environment.
- FEX reads its options once in `BTCpuProcessInit` (`runtime/build/fex-src/Source/Windows/WOW64/Module.cpp:525-531`);
  the values are cached when the context is built (`FEXCore/include/FEXCore/Config/Config.h:302-308`) and compiled into
  the translated code, so no FEX option changes while the game runs.
- `carafe.json`'s settings structs have no `#[serde(default)]` (`app/crates/carafe-core/src/settings.rs`): a new field
  without a default would make earlier records unreadable.

## What Autorun reads that Carafe does not offer

| Set | Where | Count |
|---|---|---|
| FEX | `FEX_*` lines in `<exe>.wine-nx.txt` (`fex_options.h:45-105`) | 19: 7 main, 12 advanced |
| DXVK options | `dxvk.*`, `d3d9.*`, `d3d11.*`, `dxgi.*` lines in `<exe>.wine-nx.txt` (`dxvk_options.c`) | 74 entries for 72 keys, 51 for DXVK 3.1.1 |
| DXVK and VKD3D versions | `dxvk-version`, `vkd3d-version` (`launcher_settings.h:417-436`) | the bundled 3.1.1 and 3.0.1 only; an NSP cannot download |
| Box64 | `<exe>.box64.txt` (`box64_options.h`, `wow64_box64_dynarec.c:470-498`) | 18: 6 main, 12 advanced |

- A DXVK option set to "Default" is a missing line, which keeps DXVK's own profile for the game. A line written with the
  default value would override that profile.
- FEX options 8–10 (TSO variants) matter only with TSO on; 18–19 (L1 thresholds) only with the dynamic L1 cache on.
- Autorun's launcher shows them per game in sections General, Emulation, Graphics, Controls, Frame Generation,
  Diagnostics, Library, with the advanced FEX, Box64 and DXVK options behind an "Advanced" row (`launcher.c:2311-2620`).

## How other products do it

- Steam Deck's quick access menu: a narrow panel over the running game, L1/R1 for tabs, live changes only, a per-game
  profile switch; settings that need a restart stay out of it
  ([Wccftech](https://wccftech.com/steam-deck-client-update-adds-per-game-performance-profiles/)).
- RetroArch's quick menu: full screen, pauses the game, options that need a restart say so in their label or sublabel,
  per-game overrides that store only the differences ([overrides](https://docs.libretro.com/guides/overrides/),
  [virtualjaguar #850](https://github.com/libretro/virtualjaguar-libretro/issues/850)).
- yuzu and its successors: per-game settings over the global ones, a button beside each changed value to revert it
  ([progress report](https://yuzu-mirror.github.io/entry/yuzu-progress-report-jul-2023/)); Eden on Android has a quick
  settings panel during emulation ([Eden v0.0.3](https://github.com/eden-emulator/Releases/releases/tag/v0.0.3)).
- Winlator and its forks (Wine with Box64 or FEX on phones): an in-game drawer for controls and performance, translator
  presets (Stability, Compatibility, Performance) plus custom ones, translator and DXVK choices made before launch
  ([Winlator](https://github.com/brunodev85/winlator), [Bannerlator](https://github.com/The412Banner/Bannerlator)).
- Tesla and Ultrahand on the Switch: a 448×720 panel over the running game, D-pad and A/B, L/R to jump
  ([Ultrahand](https://www.gamebrew.org/wiki/Ultrahand_Overlay_Switch)).
- Recovering from a bad setting: a trial that reverts on failure, like the display-settings timeout in Windows, and a
  safe mode after repeated crashes, like Chrome turning the GPU off after GPU-process crashes
  ([tracker](https://issuetracker.google.com/issues/492833732)).

## Risks found

- A new setting field without a default breaks reading older `carafe.json` files.
- Writing every DXVK option overrides DXVK's per-game fixes.
- A file written on the console replaces the package's file whole, so later package defaults would not arrive unless
  only the changes are kept.
- The quit combination is part of the menu's combination; holding all four for a second would quit.
- The menu's picture and font cost a few megabytes; a game close to its memory limit would lose them for good if they
  stayed loaded.
- Threads are counted against the game's budget (decision 28), so the menu cannot have its own.
