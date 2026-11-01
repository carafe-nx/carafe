# 58. DirectShow and Windows Media video play through FFmpeg

Date: 2026-11-01

## Context

Old games play their intro videos through DirectShow (`quartz.dll`): the filter graph finds a splitter for the file,
decoders and a renderer, and the game waits until the graph reports that the video has ended. Max Payne stopped right
after "Play" on its start window; its `movies/intro.mpg` is an ASF file with VC-1 video.

Three things were missing in the runtime:

- **The DirectShow catalog.** Autorun's `classes.reg` holds COM classes only. The filter categories and the filter
  data DirectShow searches (normally written by the DLLs' `DllRegisterServer`) were never there, so a graph found no
  filter at all.
- **A splitter.** Wine's DirectShow splitters, its decodebin parser and the Windows Media reader call winegstreamer's
  `wg_parser` interface. Autorun implements only the `wg_transform` part of it, on FFmpeg; every `wg_parser` call
  failed.
- **Formats.** Autorun builds FFmpeg with the MOV demuxer and H.264, AAC and WMA decoders only.

Max Payne also hung before any of this mattered: the screen-follows-window code of Autorun patch `0017` ran in the
thread DirectShow starts for its messages and moved the game's window while the game's thread waited for that very
thread, and later changed the display mode while a thread waited for a reply.

GE-Proton's video rework also plays DirectShow through FFmpeg, but it rewrites the PE side of quartz, qasf, wmvcore
and winedmo on a newer Wine and FFmpeg 8; Carafe keeps Autorun's PE DLLs and implements the interface they expect.

## Decision

- **The parser** (Autorun patch `0096`): `wg_parser` on FFmpeg in Autorun's `media_unix.c`, for 64-bit and WoW64
  callers. It reads the file through the PE side's reader thread, demuxes with libavformat, decodes with libavcodec
  and converts to the format the PE side asks for with libswscale and libswresample; with the compressed-output flag it
  passes packets through.
- **The catalog** (`runtime/dshow/add-directshow-filters.py`): the DirectShow and audio renderer categories, Wine's
  decodebin parser, both video renderers, the file source, the ACM wrapper and the AVI decompressor, with filter data
  written as quartz's filter mapper writes it.
- **Formats** (`runtime/media/configure.in`): Autorun's FFmpeg build reads its source path from
  `WINE_NX_FFMPEG_SOURCE`; Carafe points it at a `configure` that adds the ASF, AVI, MPEG, MP3, WAVE, AIFF and AU
  demuxers and the WMV/VC-1, MS-MPEG4, MPEG-1/2/4, H.263, Cinepak, MS Video 1/RLE, Indeo, MPEG audio, PCM, ADPCM and
  GSM decoders. All of them are LGPL.
- **The screen follows a window on its owner's thread only** (Autorun patch `0097`), and not while that thread waits
  for the answer to a message it sent (Autorun patch `0098` counts those waits).

## Consequences

- Max Payne plays its intro full screen and reaches its title screen.
- Media Foundation's media source in winegstreamer uses the same parser, so its videos get the same formats.
- Not covered yet: Wine's own MPEG and AVI splitters stay unregistered (decodebin opens those files and decodes them
  itself), and DMO and VfW decoders behind other graphs still go through `wg_transform`, which knows H.264 and AAC only.
