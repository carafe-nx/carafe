# 54. Wine Mono goes only into the NSPs of games that use .NET

Date: 2026-10-31

## Context

A game whose `.exe` imports `mscoree.dll` starts .NET: a .NET program, or a native one that hosts it, as Unreal
Development Kit games do. Wine answers with Wine Mono when it finds it in `C:\windows\mono\mono-2.0`. Neither
Autorun's Windows DLLs nor Carafe's runtime had it: Autorun's packaging script can stage it (`--wine-mono` in
`horizon-wine/tools/package-amd64.py`), but no build passes the option. In the lab Antichamber printed "Wine Mono is
not installed" and closed 11 s after start.

Wine Mono's runtime for 32 and 64 bits is tens of megabytes; most games never load it.

## Decision

- The runtime build downloads `wine-mono-11.3.0-arm64.tar.xz` once into `runtime/build`, checks it against the SHA-256
  that Autorun's `wine_mono_payload.py` pins, and stages it with that script into `mono/` of the runtime, next to
  `romfs/` rather than inside it, with its license.
- When a game's `.exe` imports `mscoree.dll` (`pe::uses_dotnet`), the packer lays `mono/` over the NSP's RomFS.
  Other games' NSPs stay as they were.

## Consequences

- .NET games and native games that start .NET get a runtime for it. XNA games do not: Autorun's payload leaves out
  `Microsoft.Xna` and Wine Mono's FNA.
- The runtime archive in the installer grows by Wine Mono's size; NSPs grow only for games that use .NET.
- A game that loads `mscoree.dll` itself at run time, without importing it, is not detected.
