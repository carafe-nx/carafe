# 57. Window hooks are built in

Date: 2026-11-01

The row `suspend-wakes` is gone: see [0061](0061-thread-suspension-as-on-windows.md).

## Context

Autorun's Horizon server, the server that runs inside the game's process, refused window hooks. Programs built on MFC
hook window creation with `WH_CBT` before their first window, and when the hook is refused they stop with
`CMemoryException` right at the start. Autorun patch `0085` made the server keep window hooks, first behind the
Experimental row "Window hooks", off by default (`CARAFE_WINDOW_HOOKS`).

## Decision

- **Window hooks are always on.** The Horizon server keeps the hooks a program sets on its own threads: `set_hook`,
  `remove_hook`, `start_hook_chain`, `finish_hook_chain` and `get_hook_info` work, and every thread's shared queue data
  counts the hooks that run in it, so win32u calls them
  (`runtime/patches/autorun/0085-horizon-server-window-hooks.patch`).
- **Low-level, journal and win event hooks are still refused.**
- The Experimental row and `CARAFE_WINDOW_HOOKS` are gone. Experimental keeps four rows, all off: the indirect call
  cache (`fex-call-cache`), x87 math in 32 bits (`fex-x87-single`), large pages (`large-pages`) and stopping waiting
  threads (`suspend-wakes`).

## Rejected

- **Keeping the row.** A player whose MFC game closes at once with a memory error would have to find the row first.
- **Turning hooks on only for MFC games.** The app would have to recognise MFC in the executable.

## Consequences

- MFC games get past their first window.
- A regression run in the test lab with the hooks on: OpenTTD, Need for Speed Underground and Underground 2,
  Heroes of Might and Magic IV and Need for Speed Carbon behave as before.
