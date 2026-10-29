# 48. Returns keep the miss lookup out of line

Date: 2026-10-29

Builds on [decision 46](0046-the-bridge-leaves-as-a-return.md); stage 4 of the orchestrator research, layout (c) of
[a57-translation](../research/orchestrator/findings/a57-translation.md).

## Context

On the console an instruction costs 0.5 cycles while the code fits the A57's 48 KB instruction cache, 1.6 at 64 KB
and 2.1 at 96–128 KB (W7-M1). The hot code of Carbon's race takes 74–79 KB of sampled lines, past that edge, so every
byte the hot code does not run but still carries counts.

A return in FEX pops the call-return stack, compares, and on a match returns; the lookup for a miss sits in line
between the compare and the return: 40 bytes, 64 with the profiler's miss count. Since the bridge return (decision 46)
almost no return misses. In the 256 busiest blocks of Carbon's race there were 282 return sites, 18 KB of such lookups,
and not one sample in them.

## Decision

- **FEX patch 0019, `CompactReturns`.** A return keeps four instructions in line: pop, compare, a branch on a miss,
  return. The lookup goes after the block's code, next to the jump thunks, and ends in the same return.
- **Autorun patch 0075** passes `FEX_COMPACTRETURNS` to FEX, off unless a program turns it on.
- **An Experimental row**, `fex-compact-returns`, off by default.

## Checks left

- In the lab: the usual five games with the row on, the same steps and pictures.
- On the console: Carbon driving with the row on and off; the frame rate and the main thread's share of samples.
- Next candidates of the same kind: the indirect call cache's fill and miss paths, and the entry headers of return
  points (b4, 2 cycles an entry by W7-M3).
