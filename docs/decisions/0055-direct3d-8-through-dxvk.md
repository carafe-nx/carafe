# 55. Direct3D 8 goes through DXVK for 32-bit games too

Where the modules go is replaced by [64](0064-the-runtimes-direct3d-is-the-system-direct3d.md).

Date: 2026-11-01

## Context

Decision 43 gave every game one path for Direct3D: Carafe's DXVK. The 32-bit build of DXVK still left out its
Direct3D 8 module, so a 32-bit Direct3D 8 game loaded Wine's `d3d8.dll`, which draws through wined3d and OpenGL.
In the lab Hitman 2 kept two threads busy without a first picture, Max Payne's start failed and Indiana Jones and the
Emperor's Tomb drew its menu shifted. The 64-bit build already had the module.

DXVK's `d3d8.dll` turns Direct3D 8 into Direct3D 9 and draws through its own `d3d9.dll`. The front-end thread that
decision 43 added to DXVK's Direct3D 9 stays off for Direct3D 8 devices.

## Decision

- `runtime/dxvk/build-dxvk.sh` builds `d3d8.dll` for i386 as well, and `use-dxvk.py` puts it into `drive_c/dxvk`
  with the other 32-bit modules, in the runtime manifest and in DXVK's own.
- A `d3d8.dll` in the game's folder still wins, as for every DXVK module: proxy DLLs such as an ASI loader keep
  working and load Carafe's modules themselves.

## Consequences

- 32-bit Direct3D 8 games draw through Vulkan like Direct3D 9 games: Hitman 2 and Indiana Jones reach their intro
  and title screen in the lab.
- Direct3D 8 games do not get the front-end thread; their Direct3D 9 calls run on the game's thread.
- DirectDraw and Direct3D 7 games still draw through wined3d.
