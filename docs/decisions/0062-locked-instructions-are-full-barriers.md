# 62. An x86 locked instruction is a full barrier

Date: 2026-11-04

## Context

On x86 an instruction with a `lock` prefix, and `xchg` with memory, orders every memory access before it ahead of
every access after it. Lock-free code relies on that. Wine's wined3d hands work to its command thread this way
(`dlls/wined3d/cs.c:3377-3393`, `3520-3546` at Autorun `636913ca`): the command thread sets `waiting_for_event` with
`xchg` and then reads the queue head; the program's thread writes the head with `xchg` and then reads
`waiting_for_event` with `cmpxchg`. One of the two always sees the other's write, so the command thread either finds
the work or is woken.

The Switch's Cortex-A57 has no ARMv8.1 atomic instructions, and Autorun's FEX runs without TSO emulation
(`FEX_TSOENABLED=0`). FEX then builds every locked instruction from an exclusive load-acquire and store-release pair
(`FEXCore/Source/Interface/Core/JIT/AtomicOps.cpp`), and an ordinary load after the pair may be served before the
pair's store is seen. Both threads read the old value, nobody wakes the command thread, and the program's thread
spins in `wined3d_cs_mt_finish` for good. Heroes of Might and Magic IV stopped this way at start-up in two launches
out of two once `0005` was gone (lab run `2026-11-03_21-52-21`): the main thread busy at 99.6 % of a core, the
`wined3d_cs` thread in `NtWaitForAlertByThreadId` after 51 alerts for 52 waits. It is the lost wakeup that `0005`
covered by ending every endless alert wait after 250 ms ([61](0061-thread-suspension-as-on-windows.md)).

Box64, which runs the 64-bit programs, already puts `dmb ish` before and after every locked instruction
(`horizon-wine/vendor/box64/src/dynarec/dynarec_native_pass.c:229-233`, `320-321`).

## Decision

- Without the ARMv8.1 atomics, every locked instruction FEX translates ends with `dmb ish`: compare-and-exchange of
  one and two registers, exchange, and the fetch-and-add, subtract, and, clear, or, xor and negate operations
  (`runtime/patches/fex/0024`). A failed compare-and-exchange ends with it too, as a failed `lock cmpxchg` is still a
  barrier on x86.
- FEX's handler for unaligned atomics resumes inside the sequence, before the barrier, so the barrier runs there too.

## Rejected

- **TSO emulation** (`FEX_TSOENABLED=1`). It orders the loads and stores that follow, but costs every memory access
  of the program, while only locked instructions need the barrier. On the console Need for Speed Underground 2 ran at
  31–58 fps with it, about 46 on average, and at 57–60 without it on the same runtime; Undertale, which stops at 30,
  kept its 30. It stays off in the Balanced and Fast sets and on in Reliable.
- **Waking waits on a timer** (`0005`). The wakeup was still lost; the wait only ended late.

## Consequences

- Heroes of Might and Magic IV reached its first frame in six launches out of six, 4–8 s after launch (lab run
  `2026-11-03_22-20-19`), where it had stopped in two out of two.
- A locked instruction costs one more barrier on the console; Need for Speed Underground 2's race speed stayed within
  the lab's spread.
- Ordinary loads and stores are still not ordered as on x86 while TSO emulation is off, and code that relies on that
  order can still miss a wakeup ([61](0061-thread-suspension-as-on-windows.md)).
