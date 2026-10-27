# Memory bandwidth for the CPU while the GPU draws (C4)

## Answer

The CPU and the GPU share one LPDDR4 interface, 64 bits wide (4 × 16-bit channels). At the 1331 MHz the console
reports handheld that is 1331 × 2 × 8 B = 21.3 GB/s in theory (25.6 GB/s at 1600 MHz docked); a realistic ceiling
for mixed traffic is 60–75 % of that, 13–16 GB/s (generic DRAM efficiency, not verified on this board). There is no
cache shared by the CPU and the GPU: each has its own L2, and they meet at the memory controller, which arbitrates
per client (CPU cluster, GPU, display, …) with latency-allowance and rate settings loaded with each memory-clock
table. In the NFSU2 race the GPU's traffic is estimated at 3–8 GB/s on average, and 6–15 GB/s while it is busy
(55 % of the time); the main thread, a latency-bound chain of loads, can move at most about 1.3 GB/s even if it did
nothing but miss. So bandwidth is not what limits the main thread; latency is: a DRAM miss costs at least 377 cycles,
200 ns, and 10 000 serialized misses a frame cost 2 ms, 9 % of a 47 fps frame. GPU traffic matters through that
latency: queueing behind GPU bursts raises the CPU's miss time, by an amount nobody has measured on this console
(published TX1 measurements show up to about 3× for DRAM-bound CPU code against a memory-heavy GPU kernel, a worst
case). One weak hint bounds the damage: NFSU's frame rate rose almost with the CPU clock, which a DRAM-bound thread
would not do, so at most about 20 % of the main thread waits on DRAM. The measurement that settles it is the `[A57]`
DRAM chase repeated in the race and in a loading screen, logged next to `gpu busy`.

## Evidence

### Clock and theoretical bandwidth

- The memory clock is read from `clkrst` for `PcvModuleId_EMC` (`carafe/runtime/overlay/carafe_diag.c:39-50, 355-368`)
  and is 1331 MHz handheld in every report of the race run (`logs/autorun_runtime.log:785` and every `[CLOCKS]` line;
  logs are `D:\debug_switch\carafe\nfsu2-exp9\logs`).
- Tegra X1 (Jetson TX1): "4 GB of LPDDR4 over a 4 channel x 16 bit interface with a theoretical peak memory bandwidth of
  25.6 GB/s" at 1600 MHz (NVIDIA Jetson TX1 module datasheet, found by web search; not read in full). Scaling to 1331
  MHz gives 21.3 GB/s. That the Switch uses the same 64-bit width is the common description of the console, not
  verified in a Nintendo source.
- The memory controller arbitrates by client: the memory-clock tables Atmosphère's `fusee` loads carry
  `MC_EMEM_ARB_OUTSTANDING_REQ`, `MC_MLL_MPCORER_PTSA_RATE`, `MC_LATENCY_ALLOWANCE_MPCORE_0`,
  `MC_LATENCY_ALLOWANCE_GPU_0`, `MC_LATENCY_ALLOWANCE_GPU2_0` and others per frequency
  (`Atmosphere@6b0c077:fusee/program/source/mtc/fusee_mtc_timing_table_common.hpp:330, 364, 376, 380-381`). These are
  memory-controller registers set by boot code and by the system when the memory clock changes; user mode cannot set
  them, and changing them is in the same class as changing clocks. What values Horizon uses at 1331 MHz is not read.
- No shared last-level cache between CPU and GPU on Tegra X1: the CPU's 2 MB L2 and the GPU's L2 are separate (common
  description of the chip; not verified in an NVIDIA document here). The GPU does not snoop CPU caches
  (`mesa-switch@d4a00ea0ab:src/nouveau/horizon/nouveau_horizon.h:549-556`, see
  [shared-memory-attributes.md](shared-memory-attributes.md)).

### What 377 cycles means

- `[A57]`: dependent loads, 64-byte stride, random order: L1 (16 KB) 4.0, L2 (512 KB) 34.0, "DRAM" (4 MB) 377.3
  cycles at 1887 MHz (`logs/autorun_runtime.log:838`); the benchmark is `chaseCycles` in `carafe_diag.c:161-196`.
  377.3 cycles is 200 ns.
- 377 is a lower bound for a pure DRAM miss: a 4 MB set in a 2 MB L2 that the game's threads also use still hits L2
  for part of the steps. With a hit share h, a miss costs (377 − 34h)/(1 − h): 491 cycles at h = 0.25.
- It was taken in the menu, third report, with the GPU 36 % busy at 60 fps (`logs/autorun_runtime.log:835-838`). It
  is neither an idle nor a race figure.
- A frame at 47 fps is 40 million cycles at 1887 MHz. Each 10 000 misses in a dependent chain cost 3.8 million cycles,
  2.0 ms, 9.4 % of the frame. At 1020 MHz the same 200 ns is about 204 cycles: DRAM time does not shrink with the CPU
  clock.

### How much the race moves (estimates, not verified)

CPU, upper bound: one miss in flight moves 64 B per 200 ns = 0.32 GB/s; four in flight 1.28 GB/s. That is the most the
main thread could use if every sample on a load were a DRAM miss. 67 % of its samples sit on loads and stores; how
many of those are DRAM misses is open (T1; [../../game-speed/REPORT.md](../../game-speed/REPORT.md), lines 199-207),
so the real figure is lower by that share. Writes into GPU memory (constants, UP vertices, NVK push buffers) are 1–7 MB a frame, 0.05–0.35 GB/s
([shared-memory-attributes.md](shared-memory-attributes.md)).

GPU, per frame at 1280×720 (0.92 Mpixels; `logs/SPEED2.log:671-685`), with stated assumptions:

| Traffic | Assumption | MB per frame |
|---|---|---|
| Depth read and write, colour write | overdraw 2.5–4, 12 B per pixel | 28–44 |
| Blending reads | 20–30 % of pixels blended, 4 B | 1–4 |
| Texture fetches from DRAM | 1–2 textures, 1–4 B per sample after the texture cache | 2–29 |
| Extra targets (car reflection, mirror, shadow) | NFSU2-like set | 10–30 |
| Full-screen passes (blur, glow, final) | 3–6 passes, 7.4 MB each | 22–44 |
| Vertices and indices | 1500 draws (`[D3D9 calls]`), 0.2–0.5 M vertices, 32 B | 6–16 |
| DXVK's blit to the swapchain | read and write 3.7 MB | 7 |
| Total | | 76–174 |

At 47 fps this is 3.6–8.2 GB/s, 17–38 % of 21.3 GB/s. The GPU is busy 55 % of the time at that rate
(`[CLOCKS]`, e.g. line 1484), so its demand while busy is 6–15 GB/s, up to the practical ceiling. The display adds
about 0.2 GB/s of scan-out. Maxwell's framebuffer compression and tiled rasterisation would lower the colour and depth
lines; whether NVK enables either on GM20B is not checked.

### Contention

- Two clients and one controller: the GPU's bursts sit in the same queues as the CPU's misses. The CPU's demand is
  small; the question is only how much its miss latency grows while the GPU streams.
- Published, for Tegra X1: GPU interference can raise CPU memory access delays "by up to 300 %" for working sets beyond
  the last-level cache (Cavicchioli, Capodieci, Bertogna, "Memory interference characterization between CPU cores and
  integrated GPUs in mixed-criticality platforms"; seen as a search abstract, the paper was not readable here). That
  is a synthetic GPU copy kernel against a CPU benchmark, an upper bound for a game.
- Weak evidence from the clock: NFSU raced at about 30 fps at 1020 MHz and 40–60 fps at 1785 MHz with VSync on
  ([../../game-speed/REPORT.md](../../game-speed/REPORT.md), line 125). If a share f of the main thread's time at
  1020 MHz is fixed-time DRAM waiting, a 1.75× clock gives 1/((1 − f)/1.75 + f). A gain of 1.5× or more means f ≤ 0.22.
  VSync, a different game and different scenes make this a hint, not a measurement.

### How it can be measured from user mode

- Latency under load: the `[A57]` pointer chase is user-mode code and can run at any time; repeated in a loading
  screen (GPU nearly idle) and in the race, next to the PMU's `gpu busy`, it gives latency against GPU load directly.
- CPU bandwidth: a streaming read (`ldp q` sum), write (`stp q`) and copy over 32 MB on one core, same timing method.
  The drop between loading screen and race is the bandwidth the CPU loses to the GPU.
- Memory-controller counters (Tegra activity monitor, MC statistics) are MMIO. A Horizon application cannot map them
  without kernel capabilities for I/O mappings in its NPDM, which the loader checks against the signed ACID; a
  sysmodule or a patched loader could. Not verified, and outside the one console run.

## What this means for the orchestrator

- Bandwidth is not a resource to manage for NFSU2-class games: the CPU side needs under 1.5 GB/s of 13–16 GB/s. A
  run-ahead or prefetch helper (W5) adds at most the main thread's own miss traffic again, another 0.3–1.3 GB/s; it fits.
- Latency is the resource. The main thread pays at least 200 ns per miss whatever the CPU clock, and more while the
  GPU streams. Mechanisms that remove misses from the main thread (helper prefetch, data layout, large pages) are worth
  their measured miss count × the loaded latency, not × 377.
- The orchestrator cannot arbitrate memory: the controller's settings belong to boot code and the clock tables. What
  it can shape is its own traffic: limit how many misses a helper keeps in flight, and keep helpers that stream
  (shader-cache reads, decompression) away from the main thread's busiest phase if the loaded latency proves high.
- With the race's loaded DRAM latency below about 1.2× the idle one, contention is not worth a mechanism; above 1.5×,
  cutting the main thread's misses is worth up to 1.5× more than the idle numbers say.

## Needs the console

| Measurement | How, from user mode | Decision it switches |
|---|---|---|
| `[A57] dram-idle` / `dram-race`: chase over 32 MB and 4 MB, 64-byte stride, plus 32 MB at one line per 4 KB page | the existing `chaseCycles`, run once in a loading screen and every 10th report in the race, each logged with that report's `gpu busy` | whether GPU traffic raises the main thread's miss cost; the latency factor every miss-removing mechanism is valued with |
| `[A57] stream`: GB/s for read, write, copy over 32 MB on the reporter's core, loading screen and race | `ldp`/`stp` q loops timed with `armGetSystemTick` | bandwidth budget for helper threads (W5, W10); whether contention is bandwidth or only latency |
| Docked against handheld in the same scene, if the player docks | existing `[CLOCKS]` (EMC 1600 against 1331), `[FPS]`, `[A57]` | how much frame rate depends on memory speed; nothing is changed, only recorded |
