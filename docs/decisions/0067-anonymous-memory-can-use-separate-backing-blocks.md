# Anonymous memory can use separate backing blocks

## Context

Double Agent's first campaign level fails in DXVK's anonymous
`CreateFileMappingA(INVALID_HANDLE_VALUE, ..., PAGE_READWRITE | SEC_COMMIT, ...)`.
The section allocation returns `STATUS_NO_MEMORY`; the game subsequently reads
a null pointer in `D3DDrv+0xb8e2d`. This is a RAM allocation, independent of
SaveData paths and access permissions.

The diagnostic Lab pass reproduces the console's failure on a 16 MiB section.
The aligned backing allocator has 41.52 MiB available, but its largest block is
9.59 MiB. Both the 2 MiB aligned allocation and its 4 KiB aligned retry fail.
The failure precedes descriptor creation. No Vulkan allocation fails in this
pass. The backing-map totals agree with the allocator counters.

Autorun revision `636913ca46a3638018b3ea56995d6852db9f0165`,
`dlls/ntdll/unix/horizon_memfile.h`, lines 125–137, allocates each anonymous
section as one contiguous native heap buffer. Windows only requires the section's mapped virtual
range to be contiguous; its physical pages can be separate. See Microsoft's
[virtual address spaces documentation](https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/virtual-address-spaces).

## Decision

Keep the existing contiguous allocation fast path. On `ENOMEM`, allocate
separate backing pieces, starting at 2 MiB and retrying down to a 4 KiB page.
Retain successful pieces when reducing the next allocation. The native allocator
uses separate storage for aligned allocations of at least 64 KiB and smaller
newlib allocations. Keeping earlier pieces allows one section to use both.
Release every piece on a final failure. Retain the original logical section size.

Resolve section offsets to their backing piece. Split anchors and descriptor
copies at piece boundaries. Existing aliases place those pieces next to each
other in a guest view, preserving sharing between overlapping views, descriptor
access and lifetime after the descriptor closes. Preserve `SEC_RESERVE` commit
tracking and conservative retention when the kernel refuses to remove anchors.

Pieces can have different sizes. Keep their logical offsets in allocation order
and find the containing piece with a binary search. Grow their metadata before
adding another piece; undo all allocations if that growth fails.

This is patch `0136`, confined to the anonymous section implementation. It
introduces no game-specific condition, new setting or memory-limit change.

The first validation pass confirms that the formerly failing 16 MiB section
succeeds with separate backing pieces. It then exposes the same restriction in
ordinary anonymous mappings: allocation stops at 2 MiB pieces even when roughly
31 MiB is free in smaller holes. `D3D9: InitTexture: map failed` follows failed
requests of 2112 KiB and 2176 KiB.

Patch `0137` lets the existing anonymous-mapping loop reduce pieces to 4 KiB,
rounding larger intermediate pieces to the native 64 KiB allocation granularity.
Route smaller anonymous
requests and commitment of existing reservations through the same loop. Keep
the existing contiguous fast path, protection checks, rollback and file-backed
mapping behavior. When swap support is compiled, propagate reservation state to
every resulting backing mapping.

## Rejected options

- Retrying the same large allocation with weaker alignment still requires a
  contiguous block and already fails in the captured run.
- Reducing DXVK's default section size changes a caller to accommodate a backing
  implementation restriction and leaves larger sections affected.
- Increasing the heap, changing game settings or moving saves does not correct
  the section's unnecessary contiguity requirement.
- Lazy commitment and paging are separate memory-management changes with wider
  fault-handling and storage implications.

## Validation

`make -C runtime section-memory-test` exercises the real patched section header,
the actual backing allocator and host shared mappings that make anchored source
pages inaccessible. With 24 MiB free in alternating 8 MiB holes, the original
implementation fails a 16 MiB section; the patched implementation succeeds.

The regression checks copies across piece boundaries, overlapping shared views,
closing the descriptor before unmapping, allocation and alias rollback,
reserved sections, smaller 512 KiB holes and a partial final piece. The existing
upstream section test suite also runs against the patched header. Both pass.

A second host regression extracts the actual `map_anonymous_backings` function
from the fully patched source and uses the real page pool, backing allocator
and host shared mappings. A 2112 KiB anonymous mapping fails before `0137` with
24 MiB free in 512 KiB holes. It passes after the change, including the case
where every native hole is only 64 KiB, zero-initialized guest memory, writes and
rollback after a kernel refusal. With the native allocator entirely occupied,
another case leaves 1 MiB free as separate 4 KiB pages in a pool. Both a 64 KiB
ordinary allocation and a 64 KiB section fail with a 64 KiB floor and pass with
the page-sized floor. Descriptor writes and aliases span those pages correctly,
and neighboring live pool allocations remain intact.

A regression linked to the real devkitA64 newlib and native allocator leaves
8 MiB in backing holes and over 11 MiB free in the smaller heap, with less than
64 KiB between them. A 16 MiB section fails if every retry discards earlier
pieces and starts with a uniform smaller size. Keeping successful pieces makes
it pass. The test checks descriptor copies, cleanup in both allocators and a
larger request that must still fail without leaking memory.
The SDK-linked test executes on AArch64 Linux or through `qemu-aarch64` when
available. Other build hosts compile it and explicitly report the execution
requirement; the host mapping tests still execute there.

The combined runtime with patches `0136`–`0138` and the corrected host-layout
selection was checked in Lab and on the console. The user confirmed that
Double Agent works on the console; its fresh log reaches 388 seconds and
9,503 frames without section failures or allocation failures. The Lab
campaign run instead ends in a native Eden crash during loading. These
results validate the combined changes, not each patch in isolation; see
[decision 68](0068-section-placement-and-host-address-space.md) for the
seven-scenario results, console measurements and remaining unmap rejection.
