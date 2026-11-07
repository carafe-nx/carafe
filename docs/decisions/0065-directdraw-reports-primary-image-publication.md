# 65. DirectDraw reports primary-image publication

Date: 2026-11-07

## Context

Moving dxwrapper into system DirectDraw changes HoMM IV's rendering implementation from Wine DirectDraw
to dxwrapper, Direct3D 9 and DXVK (decision 64). At Autorun `636913ca46a3638018b3ea56995d6852db9f0165`,
Carafe's window-picture notification comes from `dlls/win32u/winnx_drv.c` surface flushes (patch 0108)
and `horizon-wine/source/runtime.c` window-thread input waits (patches 0106 and 0116). Direct3D 9 rendering
through dxwrapper does not report that window-surface flush.

The loading overlay otherwise waits for a series of Vulkan presentations. A static application can publish
an image and wait for input before reaching that count. Successful Vulkan presentation can still contain
the loading overlay: `dlls/win32u/vulkan.c` submits the application image, draws the overlay through
`nx_osk_present`, and only then calls the driver's `vkQueuePresentKHR`.

## Decision

Add a process-owned publication event, passed to dxwrapper through `WINE_NX_WINDOW_PICTURE_EVENT`.
The value identifies its process and handle. dxwrapper checks the process identifier, handle width and
that its visible window is the foreground window owned by the process. Successful primary-surface drawing
arms the event before queuing Direct3D 9 presentation, including the separate presentation-thread path.

Successful `NtSetEvent` of that exact handle sets an atomic pending flag. The native Vulkan path consumes
the flag after successful scaled application-image submission, or on the direct-image path before drawing
the overlay. It updates the existing window-picture indication, which the unchanged loading code consumes.
This ordering does not treat return from asynchronous Direct3D 9 Present as GPU completion.

The event belongs to the process handle table. A missing or malformed environment value disables the
dxwrapper notification. Failed scene/draw operations do not announce publication, and failed event calls
do not set the pending flag. The notification is once per process; it is not a general multiwindow or
per-swapchain completion API.

## Rejected

- Reducing the Vulkan frame count or adding a timeout: neither reports that the application's image exists.
- Requiring another application redraw: a static application may validly wait for input after one update.
- Switching HoMM IV back to Wine DirectDraw or its local OpenGL wrapper: that would not validate system
  DirectDraw through dxwrapper and Vulkan.
- Treating successful Vulkan presentation as proof of visible game contents: the overlay and Lab's host
  presentation can still hide or retain an image.

## Validation and limits

With corrected Lab Mesa, the original runtime's static fixture still times out in `2026-11-07_07-38-03`,
and the prepublication runtime's HoMM IV still times out in `2026-11-07_07-45-15`. These archived builds
have other binary differences, so they do not isolate the notification as the only changed variable.
The earlier publication variant also passes the static fixture on corrected Mesa; its old stale-image
failure is not proof that its notification ordering failed.

The clean-build control isolates notification with one probe executable and the same runtime and Mesa.
Run `2026-11-07_08-57-55` passes its visible-image comparison (difference 0.2, tolerance 1). Run
`2026-11-07_09-01-19` clears only the publication-event environment before loading DirectDraw and times out
waiting for the first frame after 120 seconds. Both log successful initialization, Lock, Unlock and the
single primary Blt, then enter GetMessage; the disabled-notification run also logs a successful Vulkan present.
Runtime archive SHA-256 is `38b83bdf8824ded4e3cc9f1de203d0928dc6a4db005bfea7c404a269a9c36480`;
both runs report Eden image `d65f380d16f7`. Those runs had no temporary DXVK, dxwrapper or Eden diagnostics.

The investigation build passes HoMM IV's full Lab scenario in `2026-11-07_07-30-29`, including
movement, next day, save and load, with original DXVK defaults. The independent Lab Mesa stale-image defect
is documented in Lab decision 10. The clean console NSP passes that complete scenario again in
`2026-11-07_09-18-43`, using its packaged DirectDraw configuration without a Lab save fixture. Hitman 2 and
64-bit Undertale Yellow pass startup/restart checks; Antichamber fails, and A Short Hike has an inconclusive
startup comparison, as recorded in the research report. The earlier physical-console package still showed black
after successful notification and overlay removal: publication alone did not fix HoMM's startup. dxwrapper
patch `0007` resolves the separate fullscreen-reset geometry defect. The user confirmed the fixed diagnostic
package on the console; the final clean runtime passes the no-input Lab check. The
[research report](../research/homm4-system-directdraw/REPORT.md) records those distinct checks.
Multiple-window and multiple-swapchain publication are not exhaustively tested.
