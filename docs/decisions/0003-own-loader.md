# 3. Carafe's own loader in ExeFS

Date: 2026-10-04

## Context

Autorun's runtime is an `.nro`, but the system starts only NSOs from ExeFS, and the runtime expects its process handle
from a loader. Autorun's own loader, hbl, reads the `.nro` from the SD card and passes only the standard Homebrew ABI
entries.

## Decision

`runtime/loader/` is Carafe's loader, modelled on nx-hbloader (ISC, its copyright kept in `runtime/loader/LICENSE`;
its `trampoline.s` is compiled as is from `external/nx-hbloader`). It differs from nx-hbloader in that it:

- takes the runtime `.nro` and its arguments from RomFS;
- gets the process handle with `sm` closed for the moment of the exchange: an application has a limit of one session,
  and libnx keeps it busy with `sm`;
- maps `sdmc:/switch/wine/…` in the next-load request to RomFS;
- closes the application when the runtime exits without a next load, so quitting a game returns to the HOME Menu
  instead of Autorun's launcher. The exception is `/switch/wine/run-next.txt` in save data: the runtime asks to be
  started again after it installs codecs on the first run;
- keeps 96 MB out of the heap, like Autorun's loader (`HEAP_RESERVE`);
- for games with a fixed load address, reserves the image range before the runtime loads
  (see [8](0008-fixed-base-games.md)).

If the runtime exits without freeing its memory, the loader cannot unload the `.nro` and shows error `2347-0108`. This
error is kept on purpose, so a crash stays visible.

## Rejected

- Patching Autorun's hbl to read the `.nro` from RomFS: changes third-party code where our own is enough.
- Taking the process handle inside the runtime: changes in a large third-party codebase.
- Writing the loader from scratch: risk of errors in the jump trampoline.
