# 51. GPU command memory cached for every game; the translated code cache leaves the settings

Date: 2026-10-29

Supersedes the Experimental row of [decision 45](0045-gpu-command-memory-can-be-cached.md).

## Context

Carbon was driven on the console over the same stretch with Carafe's speed rows in different sets. The measure that
does not depend on the scene is the main thread's CPU time per frame (its busy share over a report divided by the
frames drawn):

| Rows on | ms per frame | Frames a second |
|---|---|---|
| none | 19.75 | 45.1 |
| large pages | 20.39 | 44.0 |
| large pages, GPU command memory cached | 16.99 | 53.0 |
| those two, compact returns, return points without the header, the indirect call cache | 17.58 | 50.6 |
| all six (two runs) | 19.13, 19.10 | 46.6, 46.9 |

FEX's own counters (blocks compiled, lookup misses) were alike in every run, so the game did the same work.

The translated code cache (`FEX_DISKCACHE`) keeps translated code on the card between runs; within a run FEX always
keeps it in memory. Its one console measurement, before the corruption fix of FEX `0004`, left the frame rate alone and
made start-up longer, 12 s to 22 s. No log since that fix shows a single cache hit.

## Decision

- **NVK's command and upload memory is CPU-cached for every game.** The overlay sets
  `NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=0` before the device is made; the row is gone. It is the one change in the table
  that moves the main thread's time on its own: 14 % less per frame.
- **The translated code cache leaves the settings.** It does nothing for the frame rate, which is what this round is
  about, and its one measurement made start-up slower. FEX `0004` and Autorun `0051` stay, off, for the next round's
  translation ahead of the game, where the cache is measured again.
- Large pages, compact returns, return points without the header store, the indirect call cache and jump tables stay
  Experimental and off: the frame times above do not show a gain for any of them alone.
