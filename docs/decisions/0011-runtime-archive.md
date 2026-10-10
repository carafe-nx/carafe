# 11. The runtime ships as one archive with checksums

Date: 2026-10-06 – 2026-10-08

## Context

The runtime is everything that goes into each NSP apart from the game: the loader and NPDM template, Autorun's
`wine-nx-runtime.nro` with the overlay and patches, about 1,700 Wine DLLs and the boot splash, 1.1 GB in total. Windows
Defender sometimes flags Wine DLLs and removes them silently.

## Decision

- **Format** (`carafe-pack/src/archive.rs`): a `CRFRUN01` header, a JSON table of contents with path, size, offset and
  SHA-256 of each file, then each file compressed with deflate on its own. 223 MB instead of 1.1 GB. A build unpacks
  only what it needs and checks every file; a mismatch stops it with `runtimeDamaged` and the list of files.
- **Wine DLLs** are Autorun's prebuilt `horizon-dlls`, checked against their `manifest.json` when the archive is made.
- **Built once** for all systems: the runtime runs on the Switch, so one Linux job in the `switch-dev` image builds it,
  and every installer gets the same folder.
- **Defender.** Carafe never adds antivirus exclusions and cannot see them without admin rights. A blocked file during
  a build gives a `blocked` error that explains how to add an exclusion; the finished NSP is safe, the DLLs are inside
  an encrypted NCA.

## Rejected

- zip or tar: new dependencies, and only a table of contents and per-file access are needed.
- One compressed stream: no way to take or check a single file.
- Loose files in the installer: Defender could remove a DLL unnoticed.
- Adding an exclusion through UAC: that is what malware does (MITRE ATT&CK T1562.001).
