# 64. The runtime's Direct3D is the system Direct3D

Date: 2026-11-05

## Context

On Windows the Direct3D a game gets is whatever sits in `system32` (`syswow64` for a 32-bit game): `d3d8.dll`,
`d3d9.dll`, `d3d11.dll`, `dxgi.dll`, `ddraw.dll`. A DLL of the same name in the game's folder is found first, which is
how wrappers and mod loaders work: d3d8to9, dgVoodoo or cnc-ddraw draw by themselves, an ASI loader loads its plugins
and hands every call on to the system DLL, which it loads by its full system path.

Autorun keeps DXVK apart from the system folders. Its runtime puts `C:\dxvk` (32-bit) or `C:\dxvk64` in front of
`system32` in a game's DLL search path, and in `WINE_NX_GRAPHICS_PATH` for `LoadLibrary`
(`horizon-wine/source/runtime.c`, `create_process_params`, and `dlls/ntdll/loader.c:3463-3527`, `get_dll_load_path`,
at Autorun `636913ca`), so it can switch between DXVK releases. Carafe's patch `0105` added `C:\dxwrapper` in front of
it for DirectDraw ([decision 59](0059-directdraw-through-dxwrapper.md)). The system folders kept Wine's own `d3d8`,
`d3d9`, `d3d10core`, `d3d11`, `dxgi` and `ddraw`, which draw through wined3d and Autorun's OpenGL. That OpenGL shows
2D pictures, but gives Direct3D games no 3D accelerator: Rig'n'Roll stopped there (decision 59).

A DLL found by name got Carafe's. A DLL loaded by its system path got Wine's: a mod loader named `d3d8.dll` in
Hitman 2's folder would have handed the game to Wine's Direct3D 8. Carafe has one DXVK for every game
([decision 43](0043-one-path-for-every-game.md)), so the folder that lets Autorun choose a release only leaves that
gap.

Wine's `wined3d.dll` stays: Wine's d3dx9, d3dcompiler and d3d10 import it for their own work, not for drawing.

## Decision

- **Carafe's DXVK modules are the system ones.** `runtime/dxvk/use-dxvk.py` puts `d3d8.dll`, `d3d9.dll`,
  `d3d10core.dll`, `d3d11.dll` and `dxgi.dll` of the 32-bit build into `drive_c/windows/syswow64` and those of the
  64-bit build into `drive_c/windows/system32`, over Wine's, and drops the `drive_c/dxvk` and `drive_c/dxvk64`
  folders. The 32-bit `d3d9.dll` still reports Direct3D 9's file version (5.3.1.904).
- **dxwrapper is the system DirectDraw.** The layout puts its `ddraw.dll` stub over Wine's in `syswow64`, with
  `dxwrapper.dll` and `dxwrapper.ini` beside it, which the stub loads from its own folder. dxwrapper loads the system
  `ddraw.dll` as its original and gets its own stub. With `Dd7to9` on, which Carafe's build sets, every DirectDraw
  export returns before it would call the original (`external/dxwrapper/ddraw/ddraw.cpp`), so nothing calls back
  into the stub.
- **The build manifest names the system DXVK.** `use-dxvk.py` writes `"system-dxvk": {"version", "revision"}` from
  Carafe's DXVK source into the runtime's `build-manifest.json`. Autorun patch `0133` takes that version as the
  installed, bundled one, so per-game DXVK settings apply as before.
- **No Direct3D folder in front of `system32`.** Patch `0105` (which replaces the old `0105` that added
  `C:\dxwrapper`) puts a DXVK folder in the search path only when it exists. The system DXVK has none, so a game
  finds its folder first, then the system folder, as on Windows. VKD3D-Proton for Direct3D 12 still comes from
  `C:\vkd3d64`.
- A DLL in the game's folder is still found first, as on Windows and as [decision 55](0055-direct3d-8-through-dxvk.md)
  keeps it.

## Consequences

- A DLL loaded by its system path, such as a mod loader's hand-off, gets Carafe's Direct3D.
- Wine's Direct3D 8, 9, 10, 11 and DirectDraw are no longer in the runtime.
- Heroes of Might and Magic IV (GOG) drew through Wine's DirectDraw before. System dxwrapper needs the primary-image
  publication notification in [decision 65](0065-directdraw-reports-primary-image-publication.md). The separate
  fullscreen-reset geometry defect is fixed by dxwrapper patch `0007`. Console confirmation and clean-build
  validation are recorded in the [research report](../research/homm4-system-directdraw/REPORT.md).
- A wrapper in the game's folder runs as on Windows on top of Carafe's Direct3D. d3d8to9 still fails to rebuild
  skinned vertex shaders, because Wine's d3dx9 disassembles the address register as `a` and `c[a.xyzw + n]` where
  Microsoft's writes `a0` and `c[a0.x + n]`, and Wine's own assembler rejects it; that is a separate item.

## Rejected

- Placing the DXVK modules in the system folders and keeping `C:\dxvk` in the search path: two copies, and the
  folder no longer has a purpose.
- Skipping wrappers found in the game's folder: Windows loads them, and a mod that needs one would break.
- A per-game DLL override in the registry: the runtime's Direct3D is the same for every game, so it belongs in the
  runtime's layout.
