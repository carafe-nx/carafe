# Committing save data while the game keeps files open

Date: 2026-10-10

## Question

Some games keep a save or settings file open for writing the whole session. The overlay committed save data only when
nothing was open for writing, so such games never committed and lost everything at restart.

## Findings

### Horizon OS

- Commit on save data fails with `ResultWriteModeFileNotClosed` (2002-6457) while any file is open with
  `OpenMode_Write`; read-only handles do not count. The server keeps a counter of writable open files and refuses the
  commit unless it is zero — Atmosphère, `fssystem_directory_savedata_filesystem.cpp`, `DoCommit`
  (<https://github.com/Atmosphere-NX/Atmosphere/blob/master/libraries/libstratosphere/source/fssystem/fssystem_directory_savedata_filesystem.cpp>);
  result codes in `fs_results.hpp`. The journaled NAND save file system is closed source; that it follows the same
  rule is not verified.
- Flushing does not help: the check counts handles, not dirty data.
- Close, commit, reopen is the known way through. Order matters: a handle reopened before the commit blocks it again.
- Uncommitted writes take journal space; once the journal is full, writes fail. Save managers (JKSV, Checkpoint)
  commit before the journal fills (<https://github.com/J-D-K/JKSV>; not checked in code).
- libnx: unmounting does not commit; `fsdevCommitDevice` must be called
  (<https://switchbrew.github.io/libnx/fs__dev_8h.html>).

### Other software

- Switch homebrew ports (ScummVM, PPSSPP, RetroArch) keep saves on the SD card and never commit
  (<https://docs.scummvm.org/en/v2.8.0/other_platforms/nintendo_switch.html>).
- Xbox GDK: commit at safe points during play and on suspend, never only at exit
  (<https://learn.microsoft.com/en-us/gaming/gdk/docs/features/common/game-save/game-saves-best-practice>).
- Steam Cloud, Ludusavi and similar tools sync at exit only; a crash loses the session.

### Carafe's runtime

- While a game runs, nothing pumps applet messages (`appletMainLoop` is called only in Autorun's recovery screen,
  `horizon-wine/source/runtime.c`), so the overlay's commit thread can read them itself. Closing a game from the HOME
  menu ends the process without `atexit`, unless the app holds `appletLockExit`. Whether sleep sends a focus change
  first is not verified.
- Before the change, write calls did not mark save data dirty: the quiet timer counted from opens and closes only.

## Set aside

- A "Saving…" icon: a commit takes a fraction of a second and the player chose to see nothing.
- Bigger journal or `fsExtendSaveDataFileSystem`: only postpones the problem.
