# 37. The Direct3D 9 front end on its own thread is built inside DXVK

Date: 2026-10-27

Research: [research/orchestrator/findings/d3d9-front-end.md](../research/orchestrator/findings/d3d9-front-end.md),
stage 2 of [decision 36](0036-orchestrator.md).

## Context

In NFSU2's race on the console the main thread is 96–99 % busy and `d3d9.dll` is 19 % of it; cores 0 and 2 are 9 %
busy. Per frame the game makes 1473 draws and about 37 000 state calls that leave a value as it was. DXVK's front end
waits for its CS thread and the GPU only a few times in 600 frames, so its work on the main thread is plain CPU work
that another core can do. The research proposed a separate i386 proxy `d3d9.dll` in front of DXVK.

## Decision

- **A patch to Carafe's DXVK** (`runtime/patches/dxvk/0007`), not a proxy DLL. The device's own entry points queue
  their call when the front end is on and the caller is the thread that created the device; a worker thread,
  `dxvk-frontend`, replays the queue into the same methods. Resources, `GetDevice` and every DXVK path stay one
  object, and no DLL is renamed or installed.
- **One gate for everything not queued:** `LockDevice` waits until the worker has emptied the queue, so a call that
  is not queued, from any thread, sees the device as the game left it. Reset and state block creation run directly.
- **Unchanged states are dropped on the game thread** against a shadow of render, sampler and texture-stage states and
  textures. State block captures and applies are queued; each block keeps a mirror so an apply updates the shadow
  without waiting.
- **DISCARD locks of dynamic buffers** hand out memory of the front end's own and queue the copy at unlock.
- **The end of an event query is queued,** and its result is read without waiting: until the queued end has run, the
  event is not signalled, as on a busy GPU. A game that checks every frame whether the GPU has caught up no longer
  empties the queue to ask.
- **Present** is queued and waits only while the previous frame is still queued: at most one frame more latency.
- **Off by default,** the Experimental row `d3d9.frontEndThread` (decision 36's rule for stages). A multithreaded,
  D3D8 or D3D9Ex device keeps the plain path.
- **The worker is a normal Wine thread at 59;** the orchestrator moves it off the main thread's core like any other.

## Rejected

- **The proxy `d3d9.dll`.** It needs a wrapper for every COM interface the game can reach (textures, surfaces,
  buffers, state blocks, queries), its own lifetime rules for them, and a rename of DXVK's DLL in every NSP, for a
  boundary that DXVK's device already has. Its one advantage, working with another Direct3D 9 implementation, does
  not apply: Carafe ships DXVK.
- **Moving only `PrepareDraw` to DXVK's CS thread.** The setters, the shadow work and the constant uploads would stay
  on the game thread, which is most of the 19 %.
