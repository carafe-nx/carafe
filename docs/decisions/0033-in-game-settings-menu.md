# 33. Game settings are changed on the console in a menu over the game

Date: 2026-10-23

## Context

A game's settings are chosen in the app before the build. Changing one meant building the NSP again on the PC and
installing it again. Autorun reads more settings for a game than Carafe offers: 19 FEX options, 74 DXVK options (51 of
them for the bundled DXVK 3.1.1), 18 Box64 options and the DXVK and VKD3D versions
([research](../research/in-game-menu/REPORT.md)). The look was chosen with the user from drafts on a canvas: the
full-screen variant in the error screen's look ([decision 32](0032-error-screen.md)).

## Decision

- **Opening.** −, +, L and R pressed together open the menu during the game and on the loading screen, and close it.
  The error screen gets a third button, "Settings", that opens the same menu. The game is not paused; while the menu is
  open, and until the buttons of the combination are released, the game gets no buttons. The quit combination (+ and −
  held for a second) does not count while L or R is held.
- **Look.** Full screen in the error screen's look: the wine-coloured gradient, M PLUS 1p, the accent `#DB6E87`, the
  ringed focus. The running game shows through the half-transparent background. The list is on the left; on the right a
  card with the description of the chosen setting, its values and when it takes effect: "now" in green, "after a
  restart" in yellow. L and R switch sections, A changes, Y resets, B goes back.
- **Sections**, the same in the menu, in the build wizard and in the app's settings for new games: Controls, Graphics,
  DirectX, Processor, System. Controls shows the whole console as the app's controller map does: mode, dead zones and
  the on-screen keyboard above, every button and stick direction around the console, a key grid for choosing a key or a
  combination of keys.
- **Settings.** Every per-game setting Autorun reads:
  - DirectX: the DXVK and VKD3D versions (only the bundled ones can be chosen) and the DXVK options in eight groups —
    memory, frames, shaders, quality, display modes, fixes, render path, what the game sees. Only the options of the
    game's Direct3D and of the chosen DXVK version are shown; before the game's Direct3D is known, all are.
  - Processor: FEX or Box64, and the options of the chosen one — seven main FEX options and twelve more in two groups
    (memory ordering; compatibility and caches), six main Box64 options and twelve more in two groups (precision and
    flags; memory and threads). An option that depends on another one is dimmed while that one is off.
  - Presets for FEX next to the options; changing an option by hand makes the preset "Custom". Balanced, the default, is
    Autorun's defaults. Reliable adds TSO, full x87 precision and `mtrack` code checks: slower, closer to x86. Fast is
    Balanced with 5000-instruction blocks and the dynamic L1 cache off: faster in heavy scenes, more memory. Fast is
    measured on several games in the lab before it ships.
- **When a change takes effect.** Button mapping, dead zones, the on-screen keyboard and the frame limit change at once.
  Everything else is read when the game starts and is marked "after a restart". "Restart" in the menu lists the changes
  before restarting.
- **Trial run**, on by default: when a game started with changed settings fails within five minutes, the error screen
  offers to bring back the settings it ran with. When a game fails twice in a row, the error screen offers to start it
  with the recommended settings.
- **Error screen.** In a menu opened from the error screen the first section is "What to try": two or three settings
  picked for the kind of failure (a crash, out of memory, out of threads). The missing-files kind gets no "Settings"
  button.
- **Loading screen.** The combinations are shown at the bottom left in a column: − and the right stick pressed — the
  keyboard; −, +, L and R — settings.
- **Where changes are kept.** The console keeps only the values changed on it, in the game's save data, and lays them
  over the package's settings at every start. Values changed on the console stay after the game is built again; values
  nobody changed come from the package, so a newer Carafe's defaults still arrive.
- **Compatibility.** Every new field of the settings has a default, and `carafe.json` written by an earlier version
  reads. DXVK options go into the settings file only when changed, so DXVK's own fixes for a game stay in force; FEX
  options are written with Autorun's defaults.
- **Resources.** The menu starts no thread. Its picture and font are loaded when it opens and freed when it closes.
- **Autorun.** Changes to Autorun are Carafe's patches in `runtime/patches/autorun/`: the combination and the exception
  for the quit combination, holding buttons back from the game, and drawing the menu's layer over the game's frames with
  transparency on the Vulkan, OpenGL and compositor paths. The blending is done on the GPU: the menu's picture is
  uploaded only when it changes, and the game's frame is never read back, because reading it back every frame made
  the game and the menu stutter on the console.
- Texts are in Russian, English and Simplified Chinese.

## Rejected

- A panel on the right, a wide panel on the left, quick tiles at the bottom: the user picked the full-screen variant
  that looks like the error screen.
- A still picture of the last frame behind the menu: the user wants the game to keep moving behind it. A solid
  background: no transparency.
- − + ZL + ZR and − + L + R: the user chose −, +, L and R.
- The HOME button: the application is not told about a short press.
- Pausing the game: a thread can be stopped only while it runs the game's own code (decision 31).
- Two steps, "Controls" and "Graphics", in the wizard: one "Settings" step with the same sections as on the console.
- Hiding the FEX, DXVK and Box64 options: the console is the only place to change them without a PC.

## Open

- Whether the app can show that a game's settings were changed on the console: it would have to read the game's save
  data from the console (not verified).
- How transparent the background is.
