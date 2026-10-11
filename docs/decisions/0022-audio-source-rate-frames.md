# 22. Audio buffer and position in the game's frames

Date: 2026-10-11

## Context

Games that play audio below 48 kHz are silent: the Switch audio backend reports its buffer in 48 kHz frames while the
game writes in its own, the first write does not fit, and the stream stays locked. Behind that, the position loses a
fraction of a frame per write, so the last block is never reported played
([research/audio-source-rate/REPORT.md](../research/audio-source-rate/REPORT.md)).

## Decision

- Patch `0013-audio-source-rate-frames` changes `horizon-wine/source/audio_unix.c`; the ring and the mixer stay in
  48 kHz frames.
- Buffer size is the ring capacity less one frame, converted to the game's rate and rounded down; padding is the held
  frames converted and rounded up. `GetBuffer` checks against the difference, so a write that passes always fits.
- Writes are converted from the running total of frames written, so no fraction is lost and the resampler keeps its
  phase between writes.
- The position is reported in the game's frames at the game's rate, never past what was written.
- A failed `ReleaseBuffer` unlocks the buffer; the conversion is done in 64 bits.

## Rejected

- Keeping the ring in the game's frames and converting in the mixer: a larger change to code shared by every stream.
- Asking Autorun to fix it first: games are silent now; the patch can go upstream as is.
