# 45. The GPU's command memory can be cached

Date: 2026-10-29

Stage 5 of the orchestrator research ([REPORT](../research/orchestrator/REPORT.md), section 3.6;
[shared-memory-attributes](../research/orchestrator/findings/shared-memory-attributes.md)).

## Context

NVK on the Switch writes its push buffers and upload streams into uncached memory by default
(`NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=1`). `dxvk-cs`, one of the busiest threads of a Direct3D 9 game, writes them 4 bytes
at a time. With `0` the same memory is cached and cleaned once at each flush. Which one is faster on the A57 is not
known: uncached stores against a cache clean per submit.

## Decision

- **An Experimental row**, `nvk-cached-commands`, off by default. When it is on, the overlay sets
  `NVK_SWITCH_CPU_WRITE_MEM_UNCACHED=0` in a constructor that runs right after the console's settings are laid over
  the package's, before the runtime starts and long before the Vulkan device is made, and logs
  `[CARAFE] settings: the GPU's command memory is cached`.
- The lab cannot tell which is faster; the console run compares `dxvk-cs`'s busy share and the frame rate with the row
  on and off.
