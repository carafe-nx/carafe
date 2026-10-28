# 40. The effect worker joins repeated passes

Date: 2026-10-28

Builds on [decision 39](0039-linked-effect-library-on-its-own-thread.md).

## Context

With the effect library on its own thread, NFSU2's race is held by that thread: `d3dx-effects` is 95–97 % busy while
the game thread is at 63–70 % and the GPU at 53–59 %. 88 % of the worker's time is inside `BeginPass`. The game draws
object after object as `BeginPass`, `CommitChanges`, draw, `EndPass` of the same pass, about 1 800 passes per frame
against 55 `Begin`, and every `BeginPass` applies every state of the pass again; 40 000 of these states per frame are
the ones already set.

The effect library's code (D3DX 9.0c in the executable) shows why joining is exact: `EndPass` touches no device state,
it only clears the in-pass flag and the list of changed parameters; parameter setters add to that list only while a
pass is open; `CommitChanges` applies the states of the parameters on the list; `BeginPass` of the pass that is already
current applies all of its states.

## Decision

- **`EndPass` followed by `BeginPass` of the same pass becomes `CommitChanges`**, when everything between them is a
  parameter change on that effect, a draw, a clear, a vertex or index stream, a buffer upload, or a device state the
  pass does not set. The pass stays open, so the parameter changes go on the list and `CommitChanges` applies them; the
  states nobody touched keep the values the pass gave them.
- **What a pass sets is recorded, not guessed.** While the effect library applies a pass, the downstream front end
  marks every state it is asked for, unchanged ones included: render states by number, constants by register, samplers
  and stages by index, the rest by kind. A state block is checked by what it captured.
- **The game never sees an effect left in a pass.** Anything else ends the pass first, as the game asked; so does every
  wait for the queue, through a record queued before the wait, because after a wait the game may call the effect
  library on its own thread.
- **An Experimental row of its own**, `d3d9.effectPassMerge`, off by default, that works only with the effect thread
  row. With the profiler on, the passes begun and joined per frame and what ended the joins are logged.

## Rejected

- **Dropping unchanged states inside the effect library**, before its call into the device. It needs the internals of
  each D3DX release, and the cost is in working the states out, not in the call.
- **Two effect workers.** The device calls of different effects must reach DXVK in order; the cores have about 30 %
  of one core free.
