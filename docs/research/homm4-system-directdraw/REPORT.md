# HoMM IV and the system DirectDraw regression

## Outcome

HoMM IV's startup regression is fixed by
[`0007`](../../../runtime/patches/dxwrapper/0007-ddraw-preserves-successful-fullscreen-reset-geometry.patch).
The user confirmed that the diagnostic package works on the physical console on 2026-11-07.
After removing temporary instrumentation and the experimental 1366x768 mode, the clean runtime
also plays the intro without input or a debugger in Lab. The clean package has not yet been run
on the console. Full gameplay has not been repeated with the final patch.

The system-DLL layout remains as specified in [decision 64](../../decisions/0064-the-runtimes-direct3d-is-the-system-direct3d.md):
DirectDraw uses dxwrapper, Direct3D uses DXVK and rendering goes through Vulkan.

## Cause

At dxwrapper `d77702d55f17`, `ddraw/IDirectDrawX.cpp:3704` resets an existing D3D9 device.
The preparation block at lines 3758-3762 then calls `SetWindowFullScreen::FullScreen` even when
that reset succeeded. DXVK has already established the fullscreen client area.

The correlated window trace and game-state comparison show this sequence:

1. The game creates its root interface using the initial client rectangle.
2. A fullscreen reset establishes a 1024x768 client area.
3. dxwrapper repeats decorated-window preparation, producing a 1024x787 client area under DXVK's
   fullscreen nonclient handling.
4. The game calls `MoveWindow` after completing its mode switch. The requested outer dimensions
   already match. dxwrapper's `GDI/WndProc.cpp:613-615` adds `SWP_NOSIZE`, so no synchronous size
   message updates the interface before the intro controller is created.
5. The controller retains width 1274. Read-only disassembly of the tested executable shows that
   the movie factory accepts widths 800, 1024 and 1280 only. It skips movie creation for 1274.

The primary copy sources are therefore black before presentation. Later size messages reach
its original window procedure and resize the root, but do not recreate the missing movie.
Successful presentation or the first-frame milestone cannot establish correct game content.

## Fix

Patch0007 limits exclusive-window preparation to cases without an existing D3D9 device.
It removes the redundant geometry mutation after a successful fullscreen reset. Initial creation,
recreation after a failed reset and fullscreen-windowed handling retain their preparation paths.

The change contains no game identification, fixed dimensions, timer, input or synthetic notification.
The game's own resize now updates the interface before it decides whether to create the movie.

## Related presentation contracts

Two separate presentation defects were established during the investigation:

- The runtime needs the primary-image publication event described in
  [decision 65](../../decisions/0065-directdraw-reports-primary-image-publication.md).
  With the same executable, runtime and Lab image, the static DirectDraw fixture displays correctly
  with that event and times out before the first frame when only the event is disabled.
- Lab's Mesa delayed-copy patch retained the last image until another presentation. Removing that
  patch makes the same one-update fixture display correctly. This is a Lab correction and does
  not enter the console package; see
  [Lab decision 10](../../../../carafe-lab/docs/decisions/0010-software-window-presentation-shows-the-current-image.md).

These contracts remain necessary for their own scenarios. Neither established that HoMM had
created its intro, so neither was sufficient to resolve the console startup regression.

## Validation

Lab runs use NCE. The HoMM startup checks send no input. Run identifiers refer to the
Lab evidence recorded on 2026-11-07.

| Check | Evidence | Result |
| --- | --- | --- |
| Failing geometry | Run `17-05-26`, runtime `1917345fd965` | Repeated preparation changes the client area; the game's resize gains `SWP_NOSIZE`; no movie is created. |
| Single behavior change | Run `17-20-01`, runtime `64653cc0fd10` | With patch `0007`, movie and Bink objects exist, RGB565 copy sources contain color, source/destination checksums match and the Buka intro is visible. |
| Repeat without debugger | Run `17-22-37`, same runtime | The intro advances to the next scene without input; normal closure succeeds. |
| Physical console | User confirmation for the package built in `17-28-03` | The diagnostic package works. |
| Clean runtime | Run `19-16-23`, runtime `ebfcc58e8080` | Intro playback and the next scene work without instrumentation, extra display mode, input or debugger; normal closure succeeds. |
| Clean RGB565 fixture | Run `19-24-19`, same clean runtime | Visible-image comparison passes with difference 0.7; normal closure succeeds. |

The final runtime SHA-256 is
`ebfcc58e80809949c54007a9b169a0de232a9eb5dbc818d2b4dbe4e070520f0e`.
The intro controller initially measures 1024x787 and the root later measures 1024x768.
Exact geometry equivalence with Wine and all mode-switch combinations are not established.

## Rejected explanations and controls

- Preserving `SW_SHOWNA` geometry alone changes the controller size to 794x556 but still creates
  no movie. Suppressing the early window show also leaves no movie. Both behavior experiments
  were removed.
- Blindly forwarding every reset-time `WM_SIZE` is not justified. The real game handler was
  identified through its dispatch table; the initially inspected handler was `WM_MOVE`.
  Wine's D3D9 comparison tests expect reset-time filtering to avoid recursive device resets.
- The 1366x768 virtual mode changes a rejected display-mode request but does not fix startup.
  It was removed; the final clean check passes without it.
- Disabling asynchronous shader compilation or the graphics pipeline library does not restore
  HoMM's missing intro. Separate fixtures expose deferred-draw loss and a Lab OpenGL sampler
  limit, but neither establishes the cause of this game-state divergence. Global defaults
  were not changed.
- A wait in the game's message loop is not evidence of deadlock. Late messages reach the game;
  the missing movie is explained by the earlier geometry and construction sequence.

Temporary API, window-message, pixel-copy, pipeline and debugger instrumentation has been removed.

## DLL migration audit

All 68 D3DX DLLs, 34 per architecture, match the initial system-migration manifest by size and
SHA-256. DXVK's five modules remain in the two system folders; 32-bit dxwrapper remains system
DirectDraw. No separate `dxvk`, `dxvk64` or `dxwrapper` folders remain. VKD3D-Proton's `vkd3d64`
folder and Wine's helper `wined3d` dependency remain as specified in decision 64.

Replacement entries in `horizon-dlls/manifest.json` retain upstream Wine source/license metadata
from their system-DLL templates. Those fields do not identify the replacement implementation;
hashes, `build-manifest.json` and runtime backend traces establish which binary runs. Correcting
that per-file provenance remains a separate packaging task.

## Earlier regression coverage

These checks precede patch `0007` and are not validation of the final clean artifact.
Run `09-27-10` uses runtime `38b83bdf8824` and Lab image `d65f380d16f7`.

| Game | Observed result | Limit |
| --- | --- | --- |
| Hitman 2 | Startup, close, restart and save-file preservation pass; system DXVK D3D8 path confirmed. | Gameplay in a level was not checked. |
| Antichamber | Wine Mono throws `NullReferenceException` during hardware survey; restart shows an invalid-handle dialog. | Not a working-game result; the restart behavior was not isolated against an earlier runtime. |
| A Short Hike | Initial startup times out; restart displays its opening logo at 92 seconds. | Overall startup check fails; the earlier comparison also lacks a working-game baseline. |
| Undertale Yellow | Run `09-49-50` passes startup, close, restart and save-file preservation; AMD64 and system DXVK D3D11 confirmed. | Gameplay beyond the introduction was not checked. |

Earlier HoMM run `09-18-43` covers the map, three hero moves, the next day and save/load restoration.
It did not assert the initial image before sending input and therefore did not cover the startup
regression. That scenario has not been repeated with the final fullscreen-reset patch.
