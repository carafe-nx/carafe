# 47. Switches jump inside the block

Date: 2026-10-29

Builds on [decision 29](0029-fex-built-with-carafe-patches.md); stage 4 of the orchestrator research, item b3 of
[a57-translation](../research/orchestrator/findings/a57-translation.md).

## Context

A 32-bit game built with MSVC turns most `switch` statements into a bounds check and an indirect jump through a
table of case addresses: `cmp reg, N; ja default; jmp [table + reg*4]`, sometimes with a byte table in between
(`movzx reg, byte [reg + bytes]`). The table sits in the code section next to the function. FEX ends the
multiblock at such a jump and finds the target in its lookup table, and every case is a block of its own with an
entry header. In NFSU2's race set the lookup sequences of 22 such jumps took 1.9 % of the samples, and the case
blocks' headers about 1.2 % more.

## Decision

- **FEX patch 0018, `JumpTables`.** When the decoder meets that pattern in a 32-bit guest, it reads the table from
  read-only executable memory, as many entries as the compare allows, and decodes the targets that lie ahead of the
  jump, within the multiblock's range, as internal blocks. The jump keeps the guest's load of the target and goes
  straight to the case whose guest address equals it, without a block entry:
  - up to four distinct cases are compared one by one. On the console (W7-M2, Carbon's start-up at 1785 MHz) an
    indirect branch took 24.9 cycles a step at random among four targets and 27.7 alternating between two, a compare
    chain 18.8 at random among four;
  - more cases are found in a host table next to the block, indexed by the guest's index, whose entries hold a case's
    guest address and the offset of its code.

  Any other target takes the lookup as before. A changed table, a wrong index or a case that was not decoded costs
  only the lookup, never a wrong jump.
- The x87 stack is written back before that branch, as it is before any other exit of the block. Backward cases are
  left to the lookup, so every loop still passes an interrupt check. The pattern is not used with the code cache
  (`FEX_DISKCACHE`), whose blocks must not hold absolute guest addresses.
- **Autorun patch 0074** passes `FEX_JUMPTABLES` to FEX, off unless a program turns it on.
- **An Experimental row**, `fex-jump-tables`, off by default, like every new behaviour-changing patch. Expected gain
  1–2 % in CPU-bound scenes of MSVC-built games.

## Checks left

- In the lab: NFSU, NFSU2, Carbon, HoMM4 and OpenTTD with the row on, the same steps and pictures as with it off,
  and a FEX-side sign that the tables were compiled.
- On the console: Carbon's driving with the row on and off, the profiler's share of the jump-table lookup sequences.
