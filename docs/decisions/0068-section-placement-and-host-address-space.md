# Section placement and host address space

## Context

The Double Agent campaign run `2026-11-09_16-30-15`, runtime `7ec09737c1ba`,
contains two distinct failures. A request for 89.125 MiB cannot find a continuous
guest range below 2 GiB. Later, a section alias destination
`[0x3a640000, 0x3a7a0000)` overlaps its source
`[0x3a661000, 0x3a7c1000)`. Horizon rejects the alias with
`InvalidCurrentMemory`; Autorun reports every alias failure as `ENOMEM`.

At Autorun revision `636913ca46a3638018b3ea56995d6852db9f0165`,
`horizon_mmap_section` in `dlls/ntdll/unix/horizon.c` registers a section view
after `horizon_memfile_map` creates its anchors. Anchor placement consults the
mapping tree, so it can reuse the pending destination. A libnx reservation
alone does not make the destination visible to that placement path.

The packer also selects a 32-bit Horizon process for every fixed x86 image
below 4 GiB. This unnecessarily places native allocations in low guest
address space when the executable can already be mapped in a 39-bit process.
The minimum mappable address in the supported 39-bit layout is 128 MiB.

## Decision

This narrows the automatic host-layout selection from
[decision 8](0008-fixed-base-games.md). Remove patch `0040`, which forced
large-address-aware allocation limits on every Win32 program. Respect the
executable's own flag instead. Carbon has not been retested as part of the
seven scenarios below, so this does not establish its compatibility after
removing that override.

Publish the complete section destination in the existing mapping tree as a
hole before creating anchors. Keep the mapping mutex held through placement
and aliasing. Change the state to aliased only after mapping succeeds. On
failure, remove the tree entry and release the reservation and section
reference. This uses the existing section states and ownership rules.

Preserve kernel error classes: an invalid memory state becomes `EEXIST`,
resource exhaustion or physical allocation failure becomes `ENOMEM`, and
other rejected alias arguments become `EINVAL`.

Select a 32-bit Horizon process only when an immovable x86 executable starts
below 128 MiB and its image fits below 4 GiB. Other executables retain the
39-bit host layout. Guest instruction set, pointer width and large-address
awareness remain properties of the Windows executable. Explicit user
overrides retain their existing behavior.

These changes address separate failures. Successful placement does not prove
that later physical allocations fit, and a larger host address space does
not make overlapping aliases valid.

## Rejected options

- Retrying with selected addresses or smaller texture sizes leaves the
  destination ownership defect in place.
- Advertising every Win32 executable as large-address-aware changes its
  Windows address contract and is not required for a larger host layout.
- Selecting a host layout by game name or manually declaring an x86 game
  to be x64 does not express the address requirement.
- Paging and compression cannot make an overlapping destination free.

## Validation

The section-placement regression extracts the actual mapping, anchor-search
and alias functions from patched Autorun. It uses the existing host shared
mapping support and rejects occupied alias destinations as Horizon does.
Before the placement change it reproduces an anchor inside the pending
destination and fails with `ENOMEM`. After the change it checks placement,
shared views, closing the descriptor first, kernel and metadata failures,
rollback, address reuse and uncommitted reservations.

Core tests cover fixed bases below, exactly at and above the 128 MiB boundary.
The existing fragmented-section, anonymous-mapping and SDK-linked allocator
regressions also pass. Game and console validation are recorded separately;
the host regressions alone do not establish that the campaign works.

The rebuilt runtime `344e0e3451a3` was tested with the updated packer in Lab.
The six HoMM IV checks use wrapper versions 1.5.0, 1.5.1 and 1.0.8, each with
`UseOpenGL=0` and `UseOpenGL=1`. Runs `2026-11-09_19-33-39`, `19-36-38`,
`19-39-20`, `19-42-16` and `19-46-44` show an advancing intro and close
normally. Runtime logs confirm Vulkan for the 1.5.0/1.5.1 zero-setting cases
and OpenGL for the one-setting cases. Run `19-45-05` exits before loading the
game image: `pthread_create` fails inside `horizon_server_connect`. This is
an unsuccessful check, not evidence of a wrapper rendering failure.
An additional run of the same 1.0.8/zero-setting case, `2026-11-09_20-05-31`,
shows an advancing intro and closes normally. It does not explain the first
bootstrap failure. Both successful 1.0.8 runs actually use OpenGL, including
the run with `UseOpenGL=0`; they do not establish a Vulkan path for that DLL.

Double Agent run `2026-11-09_19-50-42` selects a 39-bit host for the unchanged
x86 image at `0x10900000`. The test selects New Game and Easy and reaches the
Iceland loading screen; Training is not entered. Section failures remain
zero, and the last allocator snapshot reports 1,417 MiB used and 1,665 MiB
free. The earlier 89.125 MiB address-search failure, overlapping alias and
D3DDrv null access are absent from this run.

The campaign check nevertheless fails: Eden exits with SIGSEGV during
loading. Its log contains texture-cache unregister assertions and unmapped
device reads, beginning before the campaign selection. A runtime stall
snapshot places the game thread in a Vulkan call. These observations do not
identify the native crash site or establish whether the host-layout change
causes the emulator failure. That Lab run does not verify a playable
campaign. The memory regressions pass; the emulator scenario does not.

## Console follow-up

The user reports that Double Agent works on the console with the delivered
`344e0e3451a3` runtime package. The fresh log starts on 2026-11-10 at 00:22:06
under title `05cef846a22ce000`. It identifies runtime `nx-amd64-fex-2640`,
matching the Lab build, and a 39-bit host with the original fixed x86 image
at `0x10900000`.

The runtime log reaches 388 seconds and 9,503 frames, includes Iceland
resource lookups, and continues presenting after those lookups. All sampled
section-failure counters are zero, with 5,830 section views at the last
sample. No earlier alias-placement failure, allocation failure or unhandled
page fault is recorded. Sampled heap use peaks at 2,274 MiB, with at least
808 MiB free. The overlay records a requested close and successful save-data
commits with no lost reopened files. These commits alone do not verify the
game's own save/load behavior.

The log is not free of mapping errors: at 287–297 seconds,
`svcUnmapProcessCodeMemory` rejects a 16 MiB mapping with `0xd401`, so a
128 MiB fixed replacement fails with `EINVAL`. Frames continue afterward
through the final sample. The cause of that unmap rejection is unverified;
it is distinct from the previously reproduced section destination/source
overlap and is not an out-of-memory report. One ten-second frame stall
earlier in the run also recovers. Console operation is confirmed for this
test by the user and supported by these logs; longer play and the native
Eden crash remain outside the verified result.

## Removing investigation diagnostics

The console measurements above belong to the instrumented package. Before
committing, the temporary native-memory, section, progress, Vulkan, pool and
exit diagnostics and their exit wrappers are removed. The clean ELF contains
none of their diagnostic functions or markers. The rebuilt section-memory
suite, upstream compatibility tests and SDK-linked allocator test pass
without those patches.

The existing FEX fixes are already committed separately. Its generated
source cache and DLL are refreshed to the current patch set, and the native
FRNDINT regression passes 672 cases for each combination of single precision
off/on and block limits 1/1000: 2,688 cases, no failures. The resulting clean
runtime archive is `924a7915b90d`. These host checks do not constitute another
console run of that cleaned package.
