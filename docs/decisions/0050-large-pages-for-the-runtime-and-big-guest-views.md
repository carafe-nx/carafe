# 50. Large pages for the runtime and for big guest views

Date: 2026-10-29

Stage 5 of the orchestrator research, large pages of
[memory-pages](../research/orchestrator/findings/memory-pages.md).

## Context

Horizon's kernel maps a range with 2 MB (or 32 MB) entries only where the virtual address and the physical memory
behind it are aligned alike; there is no call that asks for a page size. Two of Carafe's big mappings miss that:

- the loader maps the runtime image (82 MB: Wine's unix side, Mesa, NVK, the overlay) from the 2 MB-aligned heap to an
  address that is not even 64 KB-aligned, so the whole image uses 4 KB entries;
- Wine places guest views on 64 KB boundaries, while Horizon backs every anonymous view over 2 MB with its own 2 MB-aligned
  memory, so the game's big buffers get 64 KB entries at best.

With 4 KB entries the A57's TLB reaches 4 MB; with 2 MB entries far more.

## Decision

- **The loader maps the runtime image congruent with the heap**, modulo 32 MB when the code region has room, else
  modulo 2 MB, else where it did before. Built in, without a row: the loader runs before any settings are read, and
  only the address of Carafe's own image changes; games do not see it.
- **Autorun patch 0078**: with `CARAFE_LARGE_GUEST_PAGES=1`, a view of 2 MB or more that the program lets Wine place
  starts on a 2 MB boundary when there is room, and on 64 KB otherwise.
- **An Experimental row**, `large-pages`, off by default: Carafe's overlay sets the variable from it. Aligning big
  views can leave gaps in a 32-bit game's address space, which some games already run short of.
- Expected gain 1–6 % where the data overflows the TLB's reach. The page-walk cost on the console is not measured yet.

## Checks

- In the lab (2026-10-29): the loader put the runtime at `0xf3e00000` and `0x12400000`, 2 MB-aligned; NFSU, NFSU2,
  HoMM4 and OpenTTD ran the same steps with the row on as without it. Carbon showed a black screen in its second
  variant, as it did in the run before with only FEX's rows changed: the lab's, not the row's.
- Left for the console: Carbon driving with the row on and off; the main thread's share of samples.
- Cached staging for games that read back stays out until a game that reads back is found: none of the tested games
  does (Carbon's `[D3D9 sync]` shows no read-only locks and no render-target reads), and DXVK would need explicit
  flushes for non-coherent memory.
