# 30. Carafe builds DXVK with its own patches

Date: 2026-10-20

## Context

DXVK comes prebuilt from Autorun's DLL repository: `d3d9.dll` for 32-bit programs in `C:\dxvk`, and the 64-bit
modules in `C:\dxvk64`, built by `horizon-dlls/tools/build-dxvk.py` and `build-dlls.py` from DXVK at a pinned
release, unchanged.

DXVK starts every thread through `dxvk::thread`, which throws `std::system_error` when `CreateThread` fails. Nothing
catches it, so a game that reaches the Switch's 96 threads at the wrong moment ends with exit code 3 (issue #31,
[threads research](../research/threads/REPORT.md)). The runtime makes a creation wait up to 20 s for a place
(decision 28); when none comes, DXVK still ends the game.

## Decision

- As with FEX (decision 29), the runtime build fetches DXVK at the revision Autorun's `dxvk_payload.py` names,
  applies `runtime/patches/dxvk/*.patch` and builds the 32-bit `d3d9.dll` and the 64-bit modules
  (`runtime/dxvk/build-dxvk.sh`, the `dxvk` target of `runtime/Makefile`). The layout puts them over Autorun's,
  gives the 32-bit `d3d9.dll` the DirectX 9.0c version resource Autorun gives it, and writes their sizes and SHA-256
  into the runtime's DLL manifest and `dxvk-manifest.json` (`runtime/dxvk/use-dxvk.py`).
- `0001`, `0002`: a thread DXVK cannot create is tried again, a little longer each time, for up to 120 s, and DXVK
  says so in its log every 5 s.
- `0003`: DXVK goes on with the shader compiler threads it could create. The first one waits like any other thread;
  a further one that cannot be created at once ends the list instead of the game.

## Rejected

- Catching the exception around DXVK's entry points: a device half made when a thread is refused is not usable, and
  the threads DXVK needs to draw cannot be done without.
- Waiting with no time limit: a game that holds every thread and lets none go would then stand still for good, with
  no message.
