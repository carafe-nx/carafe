# 27. Games at another resolution are fitted to the screen and centred

Date: 2026-10-18

## Context

Heroes of Might and Magic IV draws at 1024×768. On the 1280×720 screen its picture sat in the top-left corner, 1:1,
with the bottom 48 rows off the screen and the rest of the screen showing what was there before. Autorun's display
driver reports one 1280×720 mode, and nothing scaled or moved a picture of another size.

The game's own setting was windowed (`full_screen = 0`), but its Verok OpenGL wrapper (`DDRAW.dll` in the game
folder) switches the screen to 1024×768 itself. OpenTTD runs in a window whose client area is 1272×693.

## Decision

- Every game's picture is fitted into the screen whole, keeping its proportions, centred, with black bars: 4:3 games
  get 960×720 with 160 px on each side, 16:9 games fill the screen. There is no setting to stretch it or to show it
  1:1.
- It is done by win32u's mode emulation, on by default on the Switch (patch `0015`): win32u already fits an emulated
  mode into the real screen this way and maps the mouse for the game. Patch `0016` presents an emulated OpenGL window
  in its place on the screen.
- A game in a window gets the same through the screen: patch `0017` makes the emulated screen the size of the game's
  main window, so a windowed game looks like a full-screen one. There are no per-game settings, such as turning a
  game's full-screen option on.

Checked in Carafe Lab: HoMM IV at 960×720 between 160 px bars, its cursor where the screen is touched; OpenTTD's
window filling the screen; OpenTTD, Need for Speed Underground and HoMM IV pass the standard scenario, no slower than
before.

## Rejected

- Fitting only the picture, not the screen: Wine would still clip the cursor to 1280×720, and the bottom of a
  1024×768 window could not be reached.
- Turning a game's full-screen setting on per game: the player can turn it off again, every game keeps it elsewhere,
  and a game that only runs in a window would still be wrong.
- Black bars with a blurred copy of the game or of its cover: black for now; a still backdrop from the cover may come
  later.
