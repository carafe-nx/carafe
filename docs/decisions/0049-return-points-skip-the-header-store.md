# 49. Return points skip the header store

Date: 2026-10-29

Builds on [decision 48](0048-returns-keep-the-miss-lookup-out-of-line.md); stage 4 of the orchestrator research, b4
of [a57-translation](../research/orchestrator/findings/a57-translation.md).

## Context

Every entry of a FEX multiblock begins by storing the address of its block's header into the thread's state
(`adr`, `str`), then stores to the interrupt page. The header is read only when something stops the thread: a fault,
a suspend, an unaligned atomic, self-modifying code, and Carafe's profiler. Most entries are return points, the code
after a call where the callee's `ret` lands: in Carafe's race, 1678 of the 1934 entries in the 256 busiest blocks.
On the console the two stores cost 2 cycles an entry (W7-M3), and the three instructions sit in hot code that has
outgrown the instruction cache (W7-M1).

## Decision

- **FEX patch 0021, `SlimReturnPoints`.** Return points no longer store the header. The multiblock's main entry still
  does, and every entry keeps the interrupt check, so a suspend still lands at a known point.
- Whoever reads the header first syncs it from the host PC: when the PC is not inside the block the state names, FEX
  walks back from the PC to the nearest `adr`/`str` pair that stores a header and takes that block once the PC lies
  within its size. FEX's WoW64 exception handler does this before anything else, and so does `RestoreRIPFromHostPC`,
  which suspends and context reads go through. A block compiled with far jumps keeps the stores.
- **Autorun patch 0076**: the profiler does the same walk when the header in the state does not hold the sampled PC.
- **Autorun patch 0077** passes `FEX_SLIMRETURNPOINTS` to FEX, off unless a program turns it on.
- **An Experimental row**, `fex-slim-return-points`, off by default.

## Checks

- In the lab (2026-10-29, with the call cache, jump tables and compact returns on as well): NFSU, NFSU2, HoMM4 and
  OpenTTD ran the same steps as without them, and the profiler still placed 29 % of NFSU2's main thread in translated
  code. Carbon showed a black screen in both launches with them on; the same has happened in the lab before while
  the console was fine, so it waits for the console.
- Left for the console: Carbon driving with all four rows on, the main thread's share of samples; a game that
  suspends threads.
