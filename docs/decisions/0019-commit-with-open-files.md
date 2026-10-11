# 19. Save data is committed even while the game keeps files open

Date: 2026-10-10

Replaces the commit rule of [4](0004-sdmc-overlay.md). Research: [save-commit](../research/save-commit/REPORT.md).

## Context

Horizon refuses to commit save data while any file in it is open for writing. The overlay therefore waited until
nothing was open, and a game that kept a file open the whole session never committed: its saves and settings were lost
at restart.

## Decision

- To commit, the overlay closes every file the game holds open for writing in save data, commits, and reopens each
  one with its flags minus `O_CREAT`, `O_TRUNC` and `O_EXCL`, at the same offset. The game's handles stay valid.
- Each such file has its own lock; reads, writes and seeks on it wait while a commit is running. A file that cannot be
  reopened answers `EIO` and is named in the log.
- A commit runs 2 s after the last write, open or close; when half the journal is used without a pause; 10 s after
  the first unsaved change even if the game keeps writing; and at exit. Writes now count as changes.
- The commit thread reads applet messages, which nothing else does during play: it commits when the game loses focus
  (HOME) and on a close request. The overlay locks exit at start, so closing from the HOME menu waits for that commit;
  it unlocks exit afterwards and at a normal exit. Every applet message is logged with the focus state.
- Verified on the console: the close request arrives and is committed. A focus change on HOME or sleep has not been
  seen yet; whether it arrives before the game is frozen is an open question in the roadmap.
- The player sees nothing while saving.
- The log records every commit with its reason, the time since start, result, the number of reopened files and its
  duration in microseconds.

## Rejected

- A "Saving…" icon or a first-start notice: a commit is short, and the player chose a clean screen.
- Redirecting such files to the SD card: they would outlive the game and leave its save data.
- A bigger journal: postpones the failure without fixing it.
