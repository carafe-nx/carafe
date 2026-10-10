# 9. Boot splash and loading screen until the game's first frame

Date: 2026-10-04 – 2026-10-08

## Context

Between the console's boot splash and the game's first picture Windows starts up for 30–60 s. Without help the screen
stays black, and the player cannot tell a slow start from a hang.

## Decision

- **Boot splash.** Every NSP carries `assets/switch/NintendoLogo.png` (256×128) and `StartupMovie.gif` (256×80). Larger
  files crash the console. The system squeezes the logo vertically to about 0.625, so the file is stretched by 1.6 in
  advance.
- **Loading screen.** `runtime/overlay/carafe_loading.c` replaces the picture of Autorun's on-screen keyboard
  (`wine_nx_osk_*`, wrapped with `--wrap`), which Autorun already draws over every frame on all three output paths:
  OpenGL, Vulkan and the compositor. It shows the same logo and animation as the splash, in the same places, plus the
  game's icon from `/carafe/icon.jpg`, its name, the start-up stages, seconds and megabytes read. While it is shown,
  buttons do not reach the game and the mouse cursor is hidden.
- **Language** follows the console's system language: Russian or English.
- **Screen ownership.** Patches `0008`–`0011` keep the screen with the compositor until the game creates its swapchain
  or draws its first OpenGL frame. Before, Autorun gave the screen away when the game created its Vulkan surface, and
  Need for Speed Underground presents its first frame about 38 s later. The splash stays until the loading screen's
  first frame (`appletNotifyRunning` is called by Carafe). Research:
  [docs/research/loading-screen/REPORT.md](../research/loading-screen/REPORT.md).
- **When it leaves:** OpenGL — two checks in a row see bright pixels near the edges of the frame; Vulkan — after 30
  frames; without either — when the compositor shows at least 20 frames per second for two seconds; when the program
  opens the keyboard; at the latest after 180 s.

## Rejected

- Keeping the system splash until the first frame: no room for progress text, and it could stay forever.
- Its own `vi:m` display layer: never opened on the console (`0x1272`), removed.
- Drawing in the loader: a second display system, and the change of window owner hides its layer.
- A timer rule ("no OpenGL or Vulkan for 20 s"): removed the screen while OpenTTD was still loading.
- A controls hint on the loading screen.
