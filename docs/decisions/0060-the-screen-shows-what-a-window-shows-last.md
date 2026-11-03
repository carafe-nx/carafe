# 60. The screen shows what a window shows last

Date: 2026-11-03

Replaces the "windows draw at 20 frames per second" rule of [decision 9](0009-loading-screen.md).

## Context

On Windows a window has one set of pixels. GDI drawing and a windowed Direct3D present both write into it, and the
screen shows what was written last; making a device, a surface or a swapchain shows nothing by itself. On the Switch
one display layer is either Autorun's compositor, which draws the windows' GDI pixels, or a Vulkan swapchain, which
takes the whole screen when it is made and keeps it while it lives.

Indiana Jones and the Emperor's Tomb plays its intro videos with Bink, which draws each frame with GDI into the game's
window, while the game's Direct3D device has already presented two black frames and waits. Its videos were never seen,
and once seen, they sat in a corner of the screen:

- the loading screen ignored window drawing while a Vulkan surface held the screen, and its other window rule counted
  compositor frames, most of them the loading screen's own animation and the cursor;
- after the first present the swapchain kept the screen, so GDI frames went into a layer nobody drew;
- Autorun started programs with the hint to show their first window maximized, which Explorer does not give. The game
  maximized its window by the hint, Direct3D fitted it to the 640x480 mode, and the game's own `SW_SHOWNORMAL` then
  restored it to the 806x625 it was made with;
- a fullscreen window in a 4:3 display mode was drawn from the screen's left edge: win32u makes its visible rectangle
  cover the whole screen, while its surface holds the window alone.

## Decision

All rules react to events; none waits or counts frames per second.

- **A picture is what the program draws.** A window surface notes when the program drew into it (`0112`): each drawing
  operation's bounds are kept apart while the surface is locked. Backgrounds, frames and captions that
  `DefWindowProc` paints are the system's and are not counted (`0113`); a scaled surface passes the note on to the
  surface it scales into (`0114`). The window driver reports a flush of such drawing that leaves a pixel other than
  black in the client area of a shown window (`0108`).
- **The loading screen** leaves when that picture is replaced by a different one, as a video's next frame does
  (`0115`), or when the thread of the window that drew it waits for input, as a program that has started does on
  Windows (`0116`, the same moment `WaitForInputIdle` reports). The rules for OpenGL, Vulkan frames, dialogs and the
  keyboard stay.
- **The screen follows the last writer.** A picture drawn while Vulkan holds the screen takes the swapchain off it
  (`0106`, `0107`): its host swapchain is destroyed behind a gate that acquire and present pass, the compositor shows
  the windows, and the program's next acquire or present gets `VK_ERROR_OUT_OF_DATE_KHR` and makes a new swapchain,
  which takes the screen back. A refused present still waits for its semaphores and signals its fences. Programs that
  wait for presents keep their swapchain.
- **A program starts as from Explorer**, with the hint to show its window as it is (`0117`). Autorun's own rule that
  maximizes overlapped windows stays.
- **A surface is placed where its pixels belong**: in the part of the visible rectangle the window covers (`0111`).

## Consequences

- A game that shows its own splash window before its Direct3D device shows it instead of the loading screen, and then
  its black window while it loads, as on Windows.
- Games that draw only with Direct3D keep the loading screen until their Vulkan frames, as before.
- A popup window with a caption is no longer maximized at start; the screen follows its client size instead.
- Switching between a GDI picture and the game's frames makes a new swapchain each time, with one black frame between.

## Rejected

- Hiding the loading screen on a rate of changed pictures: it lost the first seconds of a video and depends on the
  game's frame rate.
- Colour rules for a picture (two colours, a distance from black): they guessed at what the program drew and took a
  program's own background fill for a picture.
- Hiding it on any changed picture: a static splash never changes.
- Giving the screen back when a swapchain has stood still for a while: a timer, and wrong for a game that presents
  rarely.
- Fitting the screen to the game's window right after its thread shows it: it followed a window the hint had put in
  the wrong place.

## Later

- Composing GDI pixels into the game's frames at present, and presenting the last frame again while the program does
  not, as Windows keeps one set of pixels per window: no screen switches and both visible at once, but a present from
  outside the game's thread on NVK's one queue. Worth it when a game needs GDI and Direct3D on screen together.
