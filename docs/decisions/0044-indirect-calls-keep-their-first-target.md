# 44. Indirect calls keep their first target

Date: 2026-10-29

Builds on [decision 29](0029-fex-built-with-carafe-patches.md); stage 4 of the orchestrator research
([REPORT](../research/orchestrator/REPORT.md), item b1 of
[a57-translation](../research/orchestrator/findings/a57-translation.md)).

## Context

FEX sends every indirect call through its L1 lookup table: a hash of the target, two loads from the thread state, a
load of the table entry, a compare and a branch. In NFSU2's race these lookups take 4.5 % of the samples of the game
thread's hot blocks; most of the call sites, COM calls from D3DX into `d3d9.dll` above all, only ever reach one target.

## Decision

- **FEX patch 0016, `IndirectCallCache`.** Each indirect call site gets a record of the guest target and its host code,
  placed after the block's jump thunks. A call to the target in the record loads the host code from the record and
  skips the table; any other target takes the lookup as before.
- **The record is filled once**, through a dispatcher stub that finds or compiles the target, writes the host code and
  then the guest address, and registers a block link. When the target's code goes, the link's delinker clears the
  guest address, and the next call fills it again.
- **An Experimental row of its own**, `fex-call-cache` (`FEX_INDIRECTCALLCACHE`), off by default. Autorun patch 0073
  passes the option on to FEX (decision 46).
- The lab cannot time the A57; it checks that the games run the same steps with the same pictures with the row on and
  off. The gain is measured on the console.

## Rejected

- **Patching the call into a direct `bl`.** It saves the indirect branch as well, but needs the linker to rewrite code
  at every call site and a delinker that rewrites it back; the record keeps the change in data.
- **Filling the record from translated code.** Without a block link the record would keep host code that has gone.
