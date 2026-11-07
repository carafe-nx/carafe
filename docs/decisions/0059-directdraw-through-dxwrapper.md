# 59. DirectDraw and Direct3D 1–7 through dxwrapper

The `C:\dxwrapper` folder and patch `0105` are replaced by
[64](0064-the-runtimes-direct3d-is-the-system-direct3d.md).

Date: 2026-11-02

## Context

DirectDraw games, and the Direct3D 1–7 games built on DirectDraw, ran on Wine's `ddraw.dll`. Wine's DirectDraw stands
on wined3d, whose default renderer is OpenGL. The Switch gives games no OpenGL context, so wined3d fell back to no
3D: Rig'n'Roll stopped with "Your computer does not have graphic accelerator".

Three ways were tried in the lab:

- **wined3d's Vulkan renderer.** Its swapchain refused 16-bit back buffers, a second swapchain could not take the
  screen, and after both were fixed the game still stopped before its first frame. The renderer is still
  experimental upstream.
- **D7VK**, Direct3D 3–7 on DXVK's Direct3D 9. It implements Direct3D only and passes DirectDraw to Wine's
  `ddraw.dll` beneath it, so the same wined3d decides: with OpenGL it held the screen and D7VK's Vulkan surface was
  refused, with no 3D the game saw no accelerator, with Vulkan the game stopped after D7VK's swapchain.
- **dxwrapper**'s Dd7to9, which implements DirectDraw 1–7 and Direct3D 1–7 itself on Direct3D 9. It needs no
  DirectDraw beneath it and draws through the system's `d3d9.dll`, which is Carafe's DXVK. Rig'n'Roll reached its menus.

## Decision

- dxwrapper is the submodule `external/dxwrapper`, pinned to the commit of its release v1.9.8700.25.
- It is built from source in its own container (`docker/dxwrapper`, `docker compose run --rm dxwrapper`): Microsoft's
  C++ compiler from Visual Studio 2022 and the Windows SDK, installed with msvc-wine and run under Wine. dxwrapper is a
  Visual Studio project and is written for that compiler. `runtime/dxwrapper/build-dxwrapper.py` reads the Release
  configuration of `dxwrapper.vcxproj` and `Stub/stub.vcxproj` and compiles them; Detours, which dxwrapper ships as a
  prebuilt library, is compiled from its submodule. The settings are dxwrapper's own `Settings.ini` with `Dd7to9`
  turned on, as its release does for DirectX 7 games.
- Carafe's changes to dxwrapper are patches in `runtime/patches/dxwrapper/` (Zlib, like dxwrapper). The build copies
  the submodule to `runtime/build/dxwrapper-src`, applies them in order there and counts their bytes in its stamp.
- `0001`: a windowed Direct3D 9 device is created on the thread that called DirectDraw; an exclusive one still goes
  to the window's thread. dxwrapper sent every creation to the window's thread and waited, so a game that opens
  DirectDraw from a worker thread while its window thread sleeps without pumping messages waited forever.
- `0002`: a windowed device leaves another thread's window style and size as they are, as `DDSCL_NORMAL` does on
  Windows. Changing that window waits for its thread in the same way.
- `0003`: a DDrawCompat hook flushes the instruction cache only for the bytes it changed. A flush of the whole
  process makes FEX throw away all the code it has translated: loading dxwrapper mid-game took 80 s, now 1.3 s.
- The layout puts `ddraw.dll` (dxwrapper's stub), `dxwrapper.dll` and `dxwrapper.ini` into `C:\dxwrapper`, and its
  license into the runtime's licenses.
- Autorun patch `0105` puts `C:\dxwrapper` before `C:\dxvk` in the search path of every 32-bit game, so a game's
  `ddraw.dll` is dxwrapper's. There is no setting: a game's own `ddraw.dll` in its folder still comes first.

## Consequences

- Direct3D 1–7 games get Carafe's DXVK with its changes and the orchestrator, like Direct3D 8 and 9 games.
- 2D DirectDraw games also go through dxwrapper and Direct3D 9 instead of Wine's DirectDraw.
- dxwrapper creates its Direct3D 9 device with `D3DCREATE_MULTITHREADED`, so DXVK's front-end thread stays off for
  these games.
- Building the runtime needs the dxwrapper container first; `npm run runtime` runs both.
