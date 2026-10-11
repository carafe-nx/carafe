# No sound in games below 48 kHz

Issue: [#14](https://github.com/carafe-nx/carafe/issues/14). Autorun read at `636913ca`.

## Symptom

Mercer Mayer's Little Critter and the Great Race (2001, 32-bit, `waveOut`) is silent. Its verbose log shows one
`WOD_PushData ReleaseBuffer failed: 88890006` (`AUDCLNT_E_BUFFER_TOO_LARGE`), then
`WOD_PushData GetBuffer failed: 88890007` (`AUDCLNT_E_OUT_OF_ORDER`) 4898 times until exit. The game's sample rate is
not in the log; not verified.

## How winmm feeds the stream

`dlls/winmm/waveform.c:1464-1583`, `WOD_PushData`: `avail_frames = bufsize - pad` from `GetBufferSize` and
`GetCurrentPadding`, capped by the queued data, then `GetBuffer(avail_frames)`, a copy, and
`ReleaseBuffer(avail_frames)`. All three are in frames of the stream's own format, as `IAudioClient` defines them.

`WOD_MarkDoneHeaders`, lines 1413-1458, marks a header done only when
`GetPosition * samples_per_sec / GetFrequency` has passed its last frame. A header that is never marked done is
never returned to the program.

## The Switch backend

`horizon-wine/source/audio_unix.c` mixes every stream to stereo 48 kHz (`NX_RATE`, line 18). The 32-bit thunks,
lines 758-985, forward to the same functions, and `nx_format`, lines 104-117, accepts any PCM rate, so a game's rate
reaches the backend unchanged.

- `nx_create_stream`, lines 208-217: the ring `capacity` is in 48 kHz frames; the scratch buffer the game writes
  into holds `capacity * source_rate / NX_RATE + 2` frames of the game's format.
- `get_buffer_size` and `get_current_padding`, lines 549 and 551, report `capacity` and `held`: 48 kHz frames.
- `nx_get_render_buffer`, line 489, checks the requested frames, which are the game's, against `capacity - held`,
  which are 48 kHz frames.
- `nx_release_render_buffer`, lines 505-507, converts the written frames to 48 kHz on their own, rounding down, and
  returns `AUDCLNT_E_BUFFER_TOO_LARGE` when they do not fit, leaving `locked` set.
- Line 487: while `locked` is set, `GetBuffer` returns `AUDCLNT_E_OUT_OF_ORDER`.
- `get_frequency` and `nx_get_position`, lines 553 and 561: the position is the 48 kHz frames played, times 4, at a
  frequency of `NX_RATE * 4`.

## Two defects

1. **Units.** A game below 48 kHz is told the buffer is larger than it is in its own frames. Its first full write
   converts to more 48 kHz frames than are free, `ReleaseBuffer` fails, and the stream stays locked for good. This
   is the log in the issue.
2. **Lost fractions.** Each write is converted on its own and rounded down, so every write loses a fraction of a
   48 kHz frame. The position, converted back to the game's frames, falls short of what was written, `winmm` never
   sees the last header played, and a program that waits for it waits forever. Hidden behind the first defect.

At 48 kHz nothing is converted and neither defect shows. Also on the same path: `GetBuffer` could hand out more
frames than the scratch buffer holds, and line 505 multiplied in 32 bits.

## Model

A C model of the backend's buffer arithmetic and of `WOD_PushData` and `WOD_MarkDoneHeaders` plays 20 blocks of
100 ms at each rate:

- unpatched: 11025, 22050 and 44100 Hz stuck after the first write with failed calls; 48000 Hz plays; 96000 Hz plays
  19 of 20 blocks;
- the first fix alone (units): 11025 and 22050 Hz play 19 of 20 blocks and wait for the last forever;
- patch `0013` (units and running conversion): all rates play 20 of 20 blocks with no failed calls.

## On the console

`tonetest.exe`, 32-bit, built with llvm-mingw, opens `waveOut` at 11025 Hz 8-bit mono, 22050 Hz 16-bit mono,
44100 Hz and 48000 Hz 16-bit stereo, plays 2 s of tone in each through four 100 ms blocks, prints progress to stdout
and gives up on a tone after 6 s. Windows 11 plays all four in full.

- Unpatched runtime, verbose log: 11025, 22050 and 44100 Hz play one block, 0.1 s, then stick; the log has the
  issue's pattern, one `ReleaseBuffer failed: 88890006` and hundreds of `GetBuffer failed: 88890007` per tone.
  48000 Hz plays in full.
- Patch `0013`: all four tones play in full and the program exits by itself; no `88890007`.

NSPs are built from `app/crates/carafe-desktop/resources/carafe-runtime.bin`, which `npm run pack` makes from the
runtime folder; a runtime rebuilt after the app started reaches an NSP only after `npm run pack`. Two earlier
"patched" console runs used the unpatched archive for this reason.

## Padding above the buffer size

With the first version of patch `0013` the verbose log still had five `GetBuffer failed: 88890006`, one or two at the
start of each tone below 48 kHz; `winmm` retried 10 ms later and the tone played in full.

The size is rounded down to the game's frames and the padding up, so after a write that fills the buffer the padding
can exceed the size. An exhaustive check of every full write over 4800-frame rings (`winmm` opens the stream with
`AC_BUFLEN`, 100 ms, `dlls/winmm/waveform.c:65, 980-983`) finds it in 8 % of cases at 11025 Hz, 16 % at 22050 Hz and
40 % at 44100 Hz, never at 48000 Hz. `WOD_PushData` computes `bufsize - pad` in `UINT32`, gets a huge number, and asks
for the whole queue. It shows at the start of a tone, when the buffer is full and the mixer has not yet taken any of
it. The model above missed it because its mixer takes a chunk before every write.

Padding is now never reported above the size, so `winmm` sees no free space and does not ask.

## Open

The first tone starts about 2 s after the program does, before and after the patch: the first stream opens the
console's audio output. Not investigated.
