# 8. Games with a fixed load address run in a 32-bit process

Date: 2026-10-07

## Context

Some 32-bit games have no relocations and must load at their base, such as 0x400000 for Need for Speed Underground. In
the usual 39-bit process there is no memory below 0x8000000, and no SVC gives it. The research is in
[docs/research/fixed-base/REPORT.md](../research/fixed-base/REPORT.md).

## Decision

- The app picks the NPDM address space from the `.exe` header: a 32-bit x86 image without relocations or with
  `RELOCS_STRIPPED`, without `DYNAMIC_BASE`, entirely below 4 GB gets type 2 (32 bit, no alias); every other game gets
  39 bit (`carafe_core::pe::required_address_space`, `npdm::set_address_space`). The kernel reads the type when it
  creates the process, so it cannot change from inside.
- Patches `0006-virtual-32bit-address-space` and `0007-low-window-32bit-guest-range` move `KUSER_SHARED_DATA` and thread
  data below 4 GB and reserve the guest part of the address space. Both act only when the address space is 4 GB or less.
- The loader checks that the image range is free and reserves it before the runtime loads
  (`runtime/loader/source/fixed_image.c`). If the kernel has taken it, the loader restarts the application with
  `RestartProgram`, at most four times in a row, with a counter in save data.
- The loader heap is 64 KB: the reservation takes a page from it, and 16 KB was not enough for the thread that fetches
  the process handle.

Tested on a console: 6 runs of 6 with the image at 0x400000, the longest session 3.8 h.

Limits: 2 GB of memory for the whole process; the guest gets between ~380 MB and ~1.4 GB below 2 GB, different on every
run.

## Rejected

- Patching Mesosphere and the system loader: it changes how the console boots.
- Shifting guest addresses in FEX and WoW64: thousands of places.
- Rebuilding relocations on the PC: completeness cannot be proven.
- One address space for the whole runtime archive: either these games or 64-bit games would break.
- A restart from inside the runtime: it happened after Wine had loaded, and its counter could be lost.
