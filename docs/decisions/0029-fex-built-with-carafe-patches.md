# 29. Carafe builds FEX's WoW64 module with its own patches

Date: 2026-10-20

The FEX revision is pinned by [decision 43](0043-one-path-for-every-game.md).

## Context

Autorun's Windows DLLs come prebuilt from its DLL repository (`external/autorun/horizon-dlls`), FEX's
`libwow64fex.dll` among them, built by `horizon-dlls/tools/build-fex.sh` from FEX at a pinned revision with
Autorun's `horizon-wine/fex/horizon.patch`.

When many threads end at once, they stand at FEX's code invalidation lock for tens of seconds
([threads research](../research/threads/REPORT.md)): freeing a thread's memory takes the lock for writing, waits for
every block being compiled and holds up every new one, though a thread's stack holds no translated code. The fix is
in FEX's source, which Carafe did not build.

## Decision

- The runtime build fetches FEX at the revision Autorun's `build-fex.sh` names, applies Autorun's patch and then
  Carafe's `runtime/patches/fex/*.patch`, and builds the WoW64 module (`runtime/fex/build-fex.sh`, the `fex` target
  of `runtime/Makefile`). The source and the build stay in `runtime/build/`.
- The layout puts that `libwow64fex.dll` over Autorun's and writes its size and SHA-256 into the runtime's copy of
  the DLL manifest (`runtime/fex/use-dll.py`), so the runtime checks pass. The ARM64EC module stays Autorun's: 32-bit
  games run through the WoW64 one.
- With Autorun's patch and no Carafe patch, the build gives Autorun's DLL to the byte, apart from the time stamp and
  padding.
- `0001`: memory that meets no executable interval is freed or flushed without the invalidation lock; FEX decodes
  guest code only from those intervals and drops a range's code when it stops being executable.
- `0002`: a thread's call-return stack is 512 KB, not 4 MB. It only predicts where a guest `ret` goes, and its
  guard page resets it when it runs over; in a 32-bit game it took 4 MB of the game's 4 GB per thread, committed.
  With 77 threads the probe had 1259 MB of address space free, against 899 MB before, and the churn phases no
  longer ran out of it.

## Rejected

- Patching the prebuilt DLL: the change is in C++ that the linker inlines; a binary patch could not be reviewed or
  carried over to the next FEX.
- Waiting for Autorun or FEX upstream: the stall is Carafe's games' problem now. FEX takes no contributions written
  with AI (its `CLAUDE.md`), so these patches stay Carafe's; Autorun can be told what they change, as an issue.
- Letting the decoder's buffer pool allocate and free outside its mutex: OpenTTD started sooner with it, but
  `Threads probe` made its second device in none of 3 runs within 180 s, against 3 of 4 in 2–3 s with `0001` alone.
- Building the ARM64EC module too: 64-bit games are not run through it in Carafe's tests yet, and it doubles the
  build.
