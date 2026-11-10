# Windows Media Player 9 compatibility: existing solutions

Date: 2026-11-08. Source research only; no runtime changes, downloads, builds or game runs.

## Finding

Existing implementations cover substantial parts of this problem. GE-Proton explicitly reports removing WMP9/WMP11
overrides for its tested games. Winetricks also provides installation recipes for Microsoft's original components.
Neither establishes universal WMP9 compatibility on Carafe. No standalone, complete WMP9 replacement suitable for
direct integration into Horizon was identified in the sources reviewed.

The recommended starting point is Carafe's existing Wine player and Windows Media reader, with targeted fixes from
GE-Proton when a required path is missing. A specific game's calls are needed to choose between that work and an
experiment with native DLLs. Adding another codec library alone cannot implement the player's COM interfaces.

## Existing options

| Option | What it supplies | What it still needs in Carafe |
| --- | --- | --- |
| Wine's built-in WMP and Windows Media libraries | Player COM interfaces and Windows Media readers | Working media backend, registration and the particular interfaces the caller uses |
| GE-Proton video rework | Wine patches that replace the GStreamer media path with FFmpeg through winedmo and repair legacy playback | Porting to Autorun's Horizon backend and checking the affected rendering and audio paths |
| Winetricks wmp9 / wmp11 | Recipes installing Microsoft's original Windows binaries and configuring overrides | Packaging, registry setup and verification of their Win32 dependencies under Autorun/FEX |
| LAV Filters | FFmpeg-based DirectShow splitters and decoders | A working DirectShow graph and renderer; it does not implement the WMP player or Windows Media Format API |

### Wine and the current Carafe path

Autorun was inspected at `636913ca46a3638018b3ea56995d6852db9f0165`; Carafe HEAD was
`52d4bbdd532a9050e7fc17f6626c954b1fb7274a`. Existing unrelated working-tree changes were preserved.

- `external/autorun/dlls/wmp/player.c:1459-1560`: `WMPControls_play` creates a DirectShow filter graph, renders the
  URL and starts it. This is an existing player implementation. Some automation methods remain `E_NOTIMPL`
  (`player.c:95-133`), so the existence of the DLL does not establish complete interface coverage.
- `external/autorun/dlls/wmvcore/wmvcore_main.c:33-69`: the synchronous reader loads Wine's media implementation.
- `external/autorun/dlls/winegstreamer/wm_reader.c:1491-1562`: that reader creates a parser, opens the input and
  enables decoded streams.
- `runtime/patches/autorun/0096-media-ffmpeg-parser-for-directshow.patch:661-666,1244`: Carafe's parser supports
  WMV/VC-1 and supplies the parser entry points. `runtime/media/configure.in:3-8` adds ASF and WMV/VC-1 codecs;
  `external/autorun/horizon-wine/cmake/Media.cmake:26` includes WMA decoders.
- `external/autorun/dlls/winegstreamer/video_decoder.c:1316` sends the DMO decoder path to `wg_transform`.
  Autorun's `horizon-wine/source/media_unix.c:145-204` selects only H.264/AAC there. Having WMV in the parser
  therefore does not establish WMV support through a separately instantiated decoder.

The [upstream Wine player source](https://github.com/wine-mirror/wine/blob/master/dlls/wmp/player.c) is the external
reference; the local revision above is authoritative for Carafe. Existing
[decision 58](../../decisions/0058-directshow-video-through-ffmpeg.md) records the parser integration and the earlier
decision to retain Wine's PE DLLs instead of importing the entire GE rework.

### GE-Proton: the closest reusable implementation

The author of [GE-Proton11-1](https://github.com/GloriousEggroll/proton-ge-custom/releases/tag/GE-Proton11-1) reports
working video in its listed games without prior WMP9/WMP11 and other DLL overrides. This is upstream test reporting,
not a Carafe test or a claim of complete WMP compatibility.

The reviewed patch headers and file summaries show why this is an integration project:

- [Media backend replacement](https://github.com/GloriousEggroll/proton-ge-custom/blob/master/patches/ge-video-rework/0001-winedmo-remove-winegstreamer-media-backend.patch),
  patch header `4a73ae599160b83ef75aa8c50ef70086e87e082e`, moves reader, parser and decoder implementations to winedmo
  and changes the Unix interface and build wiring.
- [Legacy playback changes](https://github.com/GloriousEggroll/proton-ge-custom/blob/master/patches/ge-video-rework/0004-quartz-adapt-legacy-video-paths-for-winedmo.patch),
  patch header `aa26942095323c4d2f7cef41cc43b1ecfd9efad9`, changes 34 files including quartz, qasf, wmvcore and
  DirectDraw. The header identifiers identify the reviewed patches; a complete portability audit was not performed.

Carafe substitutes its own Horizon Unix backend. Replacing a PE DLL without satisfying its Unix calls is insufficient.
Selective backports are a recommendation based on this boundary, not a verified patch plan for an unnamed game.

### Winetricks: a ready installation recipe

[Winetricks source](https://github.com/Winetricks/winetricks/blob/master/src/winetricks), inspected version string
`20260125-next`, `load_wmp9` at lines 15919-15955 and `load_wmp11` at 16010 onward, installs native Microsoft
components and configures DLL overrides. WMP9 needs installer workarounds; its additional codec installer is not
supported in a win64 prefix. WMP11 also installs prerequisites. These are recipes rather than an independent
replacement library. The native route remains unverified on Carafe; no installers or DLLs were downloaded.

### LAV Filters: useful for DirectShow, narrower than WMP

[LAV's README](https://github.com/Nevcairiel/LAVFilters/blob/master/README.md) describes splitters and decoders using
libavformat/libavcodec, registration and a custom FFmpeg build. This makes LAV a plausible experiment for a failing
DirectShow decoding path, but it supplies neither the player's COM surface nor `WMCreateReader`. Shipping its Windows
filters would also execute their codec code through the game's architecture translation instead of Carafe's existing
ARM64 FFmpeg backend; the performance impact has not been measured.

## Next decision and verification boundary

First identify whether the target calls the WMP control, Windows Media reader, DirectShow graph or a decoder directly.
Then reproduce opening, playback, audio/video completion, stop and replay on that actual path. Reuse the existing
backend and upstream fixes where possible. Native DLLs and LAV are alternatives to test when their provided API
matches the gap. No option was installed or tested here, and no implementation time estimate is established.
