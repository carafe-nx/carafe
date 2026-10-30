# 53. Pipelines are built in the background; Sarek's other choices stay Experimental

Date: 2026-10-30

Follows [decision 52](0052-the-speed-rows-are-built-in.md). The study is
[research/dxvk-sarek](../research/dxvk-sarek/REPORT.md).

## Context

A tester's Dark Souls II ran slower on 0.5.0 than on 0.4.0, with long hitches while shaders loaded: 0.4.0 let them
pick DXVK 1.10 with asynchronous pipeline compilation, and [decision 43](0043-one-path-for-every-game.md) left only
Carafe's DXVK 3.1.1, which compiles a pipeline it cannot link from shader libraries on the CS thread while drawing.
DXVK-Sarek's own ideas were carried over as Experimental rows (DXVK `0017`–`0020`, Autorun `0082`) with a counter
of pipelines built while drawing (`0016`, `0021`).

Measured:

- **Console, Carbon, all of Sarek's rows on**: no pipeline built while drawing in either race; about 1 200 draws
  skipped in the first 40 s of the first run while their pipelines compiled, 1 in the second. The compiler workers
  took under 1 % of a core each; the orchestrator left them alone (affinity 0x7, lowest priority) and the game thread
  kept 90–93 % of core 0. Main thread 16.5 ms a frame against 16.85 ms for the best run of runtime #19, within the
  run-to-run spread of about 1 ms.
- **Lab, NFSU2, two pairs with `async`**: pipelines built while drawing 19 → 0, the race picture unchanged.
- **Lab, NFSU2, one pair with every worker taking every job**: time spent building pipelines while drawing 782 → 191 ms.
- **Lab, keeping shader libraries** (`dxvk.trackPipelineLifetime = False`), three pairs: no gain beyond the noise.
- Redundant binds, the limiter's methods and its pacing were not measured on their own.

## Decision

- **Built in** (DXVK `0022`): `dxvk.shaderCompilationMethod` defaults to `async`, `dxvk.allCompilerWorkers` to
  True. The first moves to DXVK → Shaders as a row with off (`none`, DXVK's own behaviour) and `dyasync`, for a
  game whose picture misses objects; the second has no row and stays a key of the program's settings file.
- **Experimental, off**: no repeated pipeline binds, the limiter's method and its pacing.
- **Unchanged**: pipeline lifetime tracking keeps DXVK's default.

## Rejected

- **Everything on by default.** The remaining rows were only measured together with the others; repeated binds risk
  a wrong picture for a small gain, and `reactive` waits a whole frame after every present.
- **`dyasync` as the default.** It still builds the first pipeline of every shader set while drawing, the stall the
  tester saw.
- **Drawing with a neighbouring variant, as Sarek's `dyasync` does.** A variant built for other state breaks Vulkan's
  rules for dynamic state; DXVK 3's linked base pipeline already plays that part.
