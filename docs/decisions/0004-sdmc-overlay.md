# 4. Reads from RomFS, writes to save data, through an overlay under `sdmc:`

The commit rule is replaced by [19](0019-commit-with-open-files.md).

Date: 2026-10-04, updated 2026-10-08

## Context

Autorun's paths are compiled in: everything lives under `sdmc:/switch/wine`. A standalone NSP must not need files on
the SD card, and changing the paths would touch 10–15 places in Autorun's code.

## Decision

`runtime/overlay/carafe_overlay.c` replaces the `sdmc:` device inside the runtime, the same way Autorun installs its own
cache (`sd_cache.c`). It is installed by a constructor, after libnx mounts `sdmc:` and before the runtime's `main`.

- Under `sdmc:/switch/wine/…` reads go to save data, and to RomFS when the file is not in save data. Writes go to save
  data. Everything else under `sdmc:` goes to the SD card.
- Lookups ignore letter case, like FAT: Wine on Horizon expects a case-insensitive file system, while RomFS names are
  exact. The overlay builds a lower-case index of RomFS and creates new files with the spelling that already exists.
- Logs and Wine's session file `wine-nx-session-*.shm` go to `sdmc:/switch/carafe/<Title ID>/`, and so does DXVK's
  shader cache (`drive_c/users/*/AppData/Local/dxvk/…`). They stay open for writing the whole session, and save data
  cannot be committed while anything in it is open for writing.
- Save data is committed by a thread with its own static stack after 2 s without writes, and at exit, only when
  nothing in it is open for writing. The log names the files that hold a commit back (`open for writing: …`).
- The overlay shuts down through `atexit`, not a destructor: Autorun's runtime does not call destructors, and a live
  commit thread kept the loader from unloading the `.nro`.
- The overlay keeps the logs of the last five runs (`X.log` → `X.1.log` → … `X.5.log`).
- It is compiled into Autorun's build from outside: `runtime/overlay/carafe-overlay.cmake` is passed with
  `-DCMAKE_PROJECT_INCLUDE`, so Autorun's files, build files included, stay unchanged.

## Rejected

- Patching the paths in Autorun: more code to carry to every new Autorun version.
- Unpacking the runtime to the SD card: files would clash with a normal Autorun install.
- TemporaryStorage for logs: it is cleared after exit, and the logs of a crashed game would be lost.
- Committing save data only at exit: a crash would lose the whole session.
- Turning off DXVK's cache: the next starts would be slower.
