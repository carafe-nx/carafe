# W6. Speculative execution and loop splitting in the translator; T4. The upper bound of parallelism in one thread

## Answer

Thirty years of work on thread-level speculation (TLS), decoupled software pipelining (DSWP), HELIX and binary
parallelisers give one consistent picture. On integer, irregular, non-loop code, even an ideal machine finds only
2–4× of parallelism at loop and function granularity with infinite cores, and 1.7–2.3× on four (Oplinger et al.
1999). Real systems get far less. Hardware TLS on four cores gave SPECint programs +16 % on average (STAMPede). On
commodity cores with 75–110 cycles between cores, software-only schemes give nothing on such code: HELIX-RC reports
its compiler's code "performs no better than sequential execution … even with the optimistic 10-cycle core-to-core
latency". A realistic binary parallelisation study got 1.09× with hardware TLS help (VEE 2013). Software TLS costs
7–67 cycles a speculative access and about 10 000 cycles a copied page, so it pays only for coarse regions (thousands
to millions of instructions) that rarely conflict. Binary DOALL (Janus) wins on numeric loops and loses on others
(h264ref 0.76×).

For NFSU2 on four A57 cores with no HTM, generic speculation in FEX is therefore expected to give **1.00–1.05×** and
is not a stage. The closest workable approximation is narrow and coarse: a few hot loops of the game whose iterations
are independent apart from known side effects (per-car or per-object updates, culling, particle and vertex loops),
found by the T4 trace, run as DOALL or as coarse speculative chunks on two spare cores, with FEX logging only those
regions' memory accesses. If such loops are 10–25 % of the main thread and run 1.5–2× faster, the gain is
**1.04–1.14×**, on top of W1. Above that, the non-speculative "pipeline" split at the Direct3D and D3DX boundary (W1)
is the DSWP of this game: the boundary is an API whose side effects are known, so it needs no speculation.

T4 gets a concrete method: trace the main thread on a PC (the same x86 code), build the dynamic dependence graph with
renaming switches, and compute the critical path per function and loop (Kremlin's hierarchical critical path), then a
four-core model with a measured hand-off cost. The method gives the number that decides whether anything beyond W1
is worth building.

## Evidence

Numbers marked "body" were read in the paper's text; "abstract" means only the abstract or a summary was seen, not
verified. Bodies were read through a PDF-to-text proxy, so figures were extracted by a model; values marked
"approximate" looked inconsistent.

### Hardware TLS, for scale

- Hydra: Hammond, Willey, Olukotun, "Data speculation support for a chip multiprocessor", ASPLOS 1998,
  https://arsenalfc.stanford.edu/kunle/publications/hydra_ASPLOS98.pdf (body). Four cores, speculation in software
  handlers plus a modified shared L2. compress 1.09×, m88ksim 1.04×, wc 1.26×, ijpeg 1.9×, vortex 1.15×. A loop
  start costs about 75 cycles, a fork at a call about 70. Best thread size 300–3000 instructions. Speculation pays
  "only … when there is a substantial amount of medium-grained loop-level parallelism".
- STAMPede: Steffan, Colohan, Zhai, Mowry, "The STAMPede approach to thread-level speculation", ACM TOCS 23(3), 2005,
  https://www.eecg.toronto.edu/~steffan/papers/steffan_tocs_05.pdf (body). Four-core CMP, SPECint: ijpeg +84 %,
  m88ksim +56 %, average +16 %; the parallelised regions covered 32 % of the run on average (gcc 11 %, crafty 9 %,
  parser 7 %, vortex 4 %).
- Multiscalar: Sohi, Breach, Vijaykumar, ISCA 1995, https://ftp.cs.wisc.edu/sohi/papers/1995/isca.multiscalar.pdf
  (body). With a one-cycle ring between units, 4 units: gcc 1.02×, xlisp 0.91×, compress 1.17×, tomcatv 3.0×.

The hardware cases had 1–10 cycles between cores. The A57 cluster talks through the shared L2 at 34 cycles a hit,
so a hand-off is at least two L2 transactions plus coherence: about 70–150 cycles (C3, not measured).

### Software TLS without HTM: what each speculative access costs

| System | Per-access or per-page cost | Speedups and where |
|---|---|---|
| Rundberg & Stenström, JILP vol. 3, 2001, https://jilp.org/vol3/rundberg-jilp.pdf (body) | load 7–12 instructions, store 9–11, commit 10–25 per variable | numeric loops with indirect arrays: 3.3–12.5× on 8–16 CPUs; no rollbacks in them |
| SpLIP: Oancea, Mycroft, Harris, SPAA 2009, https://timharris.uk/papers/2009-spaa.pdf (body) | constant, no CAS or fence on the fast path | 77 % of manual speedup on 8 cores (TSP 4.8×, Barnes-Hut 5.3×); tolerates about 0.5 % rollbacks |
| SMTX: Raman, Kim, Oh, Lee, August, ASPLOS 2010, https://liberty.princeton.edu/Publications/asplos10_smtx.pdf (body) | versioned read 18–67 cycles, write 22–64, copy-on-write 9 600–12 000 cycles a page; two extra threads for commit | harmonic mean 13.4× on 24 cores, loops of li, parser, bzip2, gzip, hmmer (per-benchmark values approximate) |
| Privateer: Johnson et al., PLDI 2012, https://liberty.princeton.edu/Publications/pldi12_privateer.pdf (body) | one shadow byte per data byte | 11.4× on 24 cores, DOALL; 0.1 % injected misspeculation halves most speedups |
| BOP: Ding et al., PLDI 2007, https://www.cs.rochester.edu/u/cding/Documents/Publications/pldi07.pdf (body) | processes and page protection; 2 600–4 500 pages copied per commit in gzip | gzip 1.20× / 1.61× / 2.08× on 2 / 4 / 8 CPUs, parser 1.13 / 1.62 / 2.12; regions of about 10 MB of work |
| CorD: Tian, Feng, Nagarajan, Gupta, MICRO 2008, https://llvm.org/pubs/2008-11-MICRO-CopyOrDiscard.pdf (body) | at most 7 % of instructions for the model | 3.7–7.8× on 8 threads, selected hot loops covering 31–100 % of the run |
| Cintra & Llanos, PPoPP 2003 (abstract) | — | 71 % of manual speedup with few violations |

The pattern: software TLS works when regions are large and conflicts are rare, and when somebody (a programmer or a
profile) has chosen the loops. On four cores the integer programs get 1.6× at best (BOP), for loops the authors
picked.

### DSWP: a pipeline across cores

- Ottoni, Rangan, Stoler, August, "Automatic thread extraction with decoupled software pipelining", MICRO 2005,
  https://liberty.princeton.edu/Publications/micro05_dswp.pdf (body). Two Itanium 2 cores with a hardware
  synchronisation array (one cycle to produce or consume). Loops +14.4 % automatic, +19.4 % manual; whole programs
  +6.6 % / +9.2 %. Re-run at 10 and 100 cycles of latency: "not very sensitive to the communication latency".
- Rangan et al., "Support for high-frequency streaming in CMPs", MICRO 2006,
  https://liberty.princeton.edu/Publications/micro06_cmpcomm.pdf (body). DSWP threads exchange a value every 5–20
  instructions. A software queue costs about 10 instructions per operation. Latency is tolerated; the per-operation
  cost is not: with software queues DSWP was sometimes slower than one thread.
- SpecDSWP (Vachharajani et al., PACT 2007, https://liberty.princeton.edu/Publications/pact07_specdswp.pdf, body):
  four cores with one-cycle queues and versioned memory: loops 1.24–2.24×, average 1.40×; misspeculation 0.2–15.9 %.
- PS-DSWP (Raman et al., CGO 2008, https://liberty.princeton.edu/Publications/cgo08_psdswp.pdf, body): six cores,
  geomean +114 %.
- Bridges et al., "Revisiting the sequential programming model for multi-core", MICRO 2007,
  https://liberty.princeton.edu/Publications/micro07_scale.pdf (body): geomean 5.54× on SPECint, but with 60 changed
  source lines, versioned-memory hardware and 256 hardware queues, simulated.

What it means here: a DSWP stage boundary must carry rare, large messages. That is exactly an API boundary such as
Direct3D 9: one record per call, hundreds of instructions of work behind each. W1 is DSWP with the cut placed by hand
at the one place where the side effects are documented.

### HELIX: iterations across cores

- Campanoni et al., "HELIX: automatic parallelization of irregular programs for chip multiprocessing", CGO 2012,
  https://www.cl.cam.ac.uk/~tmj32/papers/docs/campanoni12-cgo.pdf (body). Measured on a six-core i7-980X: geomean
  2.25× over 13 SPEC2000 programs, gzip and mcf about 1.1×. Signals between cores took 110 cycles; HELIX hid them with
  a helper thread on each core's SMT sibling that prefetched the signals into L1. Signalling, not data, was the main
  cost.
- Campanoni et al., "HELIX-RC", ISCA 2014, https://skanev.org/papers/isca14helixrc.pdf (body). 6.85× on 16 simulated
  in-order cores with a 1 KB ring cache and one-cycle hops. More than half of the hot SPECint loop iterations end
  within 25 cycles. Real core-to-core latencies quoted: Ivy Bridge 75, Sandy Bridge 95, Nehalem 110 cycles, "much
  too long for the majority of these short loops". Without the ring cache the new compiler's code "performs no better
  than sequential execution … even with the optimistic 10-cycle core-to-core latency".

The A57 has no SMT and no ring cache, so HELIX's own workaround is not available.

### Parallelising binaries

- Janus: Zhou & Jones, "Janus: statically-driven and profile-guided automatic dynamic binary parallelisation", CGO
  2019, https://www.cl.cam.ac.uk/~tmj32/papers/docs/zhou19-cgo.pdf, code https://github.com/CompArchCam/Janus (body).
  DynamoRIO with a static analyser that emits rewrite rules. DOALL loops only; only 9 of 25 SPEC2006 programs spend
  20 % or more in such loops, almost all floating point. Eight threads: 2.1× average, libquantum 6.0×, lbm 5.8×,
  h264ref 0.76×. Its software TM checkpoints registers at `TX_START`, logs heap and non-local stack accesses at word
  granularity, buffers stores, validates by value at an in-order commit. It rejects loops with I/O, system calls,
  exceptions or calls that do not return.
- SecondWrite: Kotha et al., "Automatic parallelization in a binary rewriter", MICRO 2010,
  https://user.eng.umd.edu/~barua/micro10-aparna.pdf (body). Affine loops only: 5.1× from the binary against 5.7×
  from source on 8 threads.
- Pradelle, Ketterlin, Clauss, "Polyhedral parallelization of binary code", TACO 8(4), 2012,
  https://inria.hal.science/hal-00664370 (abstract). Affine loop nests lifted to C.
- Yang, Skadron et al., "Feasibility of dynamic binary parallelization", HotPar 2011,
  https://www.cs.virginia.edu/~skadron/Papers/yang_hotpar11.pdf (body). A limit study with zero overhead: at least
  1.27× for 5 of 7 integer programs; only 61 % of integer instructions lie in predictable hot traces.
- Edler von Koch & Franke, "Limits of region-based dynamic binary parallelization", VEE 2013,
  https://groups.inf.ed.ac.uk/pasta/pub_VEE_2013_LIMITS_BIN_PARAL.html (abstract). Up to 54 % shorter critical path in
  SPEC2006; a realistic scheme with light hardware TLS: 1.09× on average. This is the closest case to FEX.
- RASP: Hertzberg & Olukotun, CGO 2011 (abstract): +49 % on SPEC2006 integer with four simulated in-order cores and
  hardware TLS, in an x86 translator.
- Wang et al., "Dynamic parallelization of single-threaded binary programs using speculative slicing", ICS 2009, and
  Yardimci & Franz, CF 2006 (abstracts only, no numbers seen).

### Running ahead and method-level speculation

- MSSP: Zilles & Sohi, "Master/slave speculative parallelization", MICRO 2002,
  https://zilles.cs.illinois.edu/papers/mssp.micro2002.pdf (abstract). A distilled copy of the program runs ahead
  and predicts live-ins for slave cores: up to 1.7×, harmonic mean 1.25× on SPEC2000 integer, simulated hardware.
- Chen & Olukotun, "Exploiting method-level parallelism in single-threaded Java programs", PACT 1998,
  https://arsenalfc.stanford.edu/kunle/publications/hydra_PACT98.pdf (body). Four cores: control-flow programs
  1.1–1.4×; fork and checkpoint about 50 cycles.
- Jrpm: Chen & Olukotun, ISCA 2003, https://arsenalfc.stanford.edu/kunle/publications/hydra_ISCA03.pdf (body).
  Integer 1.5–2.5× on four cores, with hardware support (start of a thread 23 cycles).
- Warg & Stenström, "Limits on speculative module-level parallelism", PACT 2001, numbers from Warg's licentiate
  thesis, https://warg.org/fredrik/publ/lic/warg_lic.pdf (body, approximate). With perfect value prediction and no
  overhead: harmonic mean about 3.4 (C programs); gcc and compress 1.16. At 100 cycles of overhead per thread several
  programs slow down.

### Limit studies (the T4 method)

- Wall, "Limits of instruction-level parallelism", ASPLOS 1991 / DEC WRL 93/6,
  https://users.cs.utah.edu/~rajeev/cs7810/papers/wall93.pdf (body, read off figures). Greedy scheduling of a trace,
  2K-instruction window, perfect renaming and alias analysis; models differ in branch prediction. Integer programs
  stay at about 3–15 even in strong models; numeric ones reach 45–500.
- Lam & Wilson, "Limits of control flow on parallelism", ISCA 1992,
  https://ece.northeastern.edu/courses/ece3391/papers/ILP.pdf (body). True dependences only, perfect memory
  disambiguation. Non-numeric harmonic means: waiting at every branch 2.14; speculation 6.80; speculation, control
  dependence and multiple flows of control 39.6; oracle 158.
- Austin & Sohi, "Dynamic dependency analysis of ordinary programs", ISCA 1992,
  https://ftp.cs.wisc.edu/sohi/papers/1992/isca.ilp.pdf (abstract). The "Paragraph" tool: a dynamic dependence graph
  from a serial trace, each operation at its earliest level, registers and memory renamed or not. Parallelism is
  bursty; renaming memory as well as registers matters.
- Postiff, Greene, Tyson, Mudge, CAN 27(1), 1999 (abstract). Dependences through the stack pointer from call linkage
  hide much of the parallelism. This weighs heavily on 32-bit MSVC code, where ESP chains every push, call and spill.
- Mak & Mycroft, WODA 2009, details in Mak's thesis UCAM-CL-TR-796,
  https://www.cl.cam.ac.uk/techreports/UCAM-CL-TR-796.pdf (body). With control dependences mostly under 10; true data
  dependences only often above 100; removing stack dependences about doubles it. "Control dependences form the biggest
  obstacle."
- Oplinger, Heine, Lam, "In search of speculative thread-level parallelism", PACT 1999,
  https://suif.stanford.edu/papers/oplinger99.pdf (body). An optimal machine with no overhead and perfect value
  prediction, SPECint95: loops 2.6×, procedures 2.3×, both 3.9× with infinite cores; 2.3× on four cores; 1.7× on four
  with a base machine that rolls back.
- Kreaseck, Tullsen, Calder, "Limits of task-based parallelism in irregular applications", ISHPC 2000,
  https://cseweb.ucsd.edu/~calder/papers/ISHPC2K-TASK.pdf (body). Only 7–22 % of SPECint95's dynamic instructions lie
  in tasks independent through memory.
- Kejariwal et al., ICS 2006 (abstract): oracle loop-level TLS on SPEC2000, +18 % geometric mean.
- Kremlin: Garcia, Jeon, Louie, Taylor, "Kremlin: rethinking and rebooting gprof for the multicore age", PLDI 2011,
  https://michaeltaylor.org/papers/pldi-garcia-kremlin-2011.pdf (body). Hierarchical critical path analysis (HCPA):
  shadow-memory timestamps per region level, each function and loop region starting at time 0; anti and output
  dependences, induction variables and reductions removed. Self-parallelism = sum of the children's critical paths
  over the region's own critical path. About 50× slower than gprof.
- Kismet: Jeon, Garcia, Louie, Taylor, OOPSLA 2011, https://bsg.ai/papers/oopsla-jeon-kismet-2011.pdf (body). HCPA
  plus a per-region time model with core count, overhead and cache time. SPECint2000 loop parallelism with zero
  overhead plateaus at 2–4×. No public source code found for Kremlin or Kismet.
- Alchemist: Zhang, Navabi, Jagannathan, CGO 2009, https://www.cs.purdue.edu/homes/xyzhang/Comp/cgo09.pdf (body).
  Valgrind; dependence distance per procedure, loop and conditional through an execution index tree; 166–712×
  slowdown.
- Loopapalooza, ISPASS 2021,
  https://pure.manchester.ac.uk/ws/files/191555438/loopapalooza_ISPASS_2021_authors.pdf (body). LLVM; relaxations
  step by step (reductions, loop-carried registers, calls, DOALL to HELIX): SPEC2006 integer upper bound 7.2×.
- DiscoPoP (https://github.com/discopop-project/discopop) is open but works on LLVM IR from source, like Kremlin and
  Loopapalooza. None of these tools reads a 32-bit Windows binary.

### The tracer for a binary

DynamoRIO's drmemtrace records every instruction fetch and data access of a thread, with markers for system calls and
for chosen functions (`-record_function name|args` writes `TRACE_MARKER_TYPE_FUNC_ID`, arguments and return values),
and with instruction encodings in offline traces (`OFFLINE_FILE_TYPE_ENCODINGS`), so an analysis tool can decode
register reads and writes (https://dynamorio.org/sec_drcachesim_format.html,
https://dynamorio.org/sec_drcachesim_funcs.html, https://dynamorio.org/sec_drcachesim_newtool.html,
https://dynamorio.org/sec_drcachesim_partial.html). The same trace feeds `drcachesim`, a cache simulator that can be
set to the A57's L1 and L2 (W5, T1). DynamoRIO runs 32-bit Windows programs; whether it attaches cleanly to NFSU2 with
DXVK's `d3d9.dll` is not verified. Using it needs a download, to be agreed first.

## What this means for the orchestrator

1. **No generic TLS in FEX.** The guest's code (MSVC 2004, x87, eight registers, stack spills everywhere) is the case
   every study says does not pay: short regions, stack-pointer chains, frequent calls into system DLLs. A generic
   speculative mode would cost 7–67 cycles per logged access on a thread where 40 % of the instructions touch memory.
2. **The non-speculative split first.** W1's front end is the pipeline stage that the DSWP papers would choose: one
   record per call, hundreds to thousands of instructions behind each, side effects documented by the API. It needs no
   rollback.
3. **A narrow "region parallel" mode in FEX, only if T4 shows candidates.** The parts and the layer each lives in:
   - Region selection (orchestrator, per-game profile on the SD card): guest address ranges of loops that T4 marked
     DOALL or speculatively DOALL with conflicts below 1 % of iterations, at least 20 000 instructions of work per
     invocation, no system calls inside.
   - Speculative compile (FEX, a new IR pass, a patch in `runtime/patches/fex`): the region's blocks are compiled a
     second time with every guest load recorded (address and value, for value-based validation like Janus) and every
     store buffered in a per-thread write log; reads first look up the log. Accesses to the thread's own new stack
     frames are left alone, as Janus does.
   - Checkpoint and rollback (FEX): copy of the guest state (`CpuStateFrame`: GPRs, flags, x87 and SSE registers, a
     few hundred bytes, tens of cycles) at the start of each chunk; on a conflict the chunk's log is dropped and the
     chunk runs again on the main thread.
   - Stop points (FEX and the runtime): `SyscallOp` (the `int 0x2e` bridge to Wine), any call whose target is outside
     the game's image (system DLLs, `d3d9.dll`), `lock`-prefixed instructions, self-modifying code, any guest fault.
     A speculative thread that reaches one stops and waits to become the oldest. Direct3D calls inside a region could
     instead be buffered as W1 records and committed in order; that is the one side effect the runtime can buffer.
   - Commit (FEX): in order; validation compares the recorded values with memory; the write log is applied.
   - Workers: two threads on spare cores, woken per region invocation, not per iteration, chunking iterations so each
     chunk is 5 000 or more instructions.
4. **The expected gain** is (from T4) the coverage C of such loops times what they gain on two or three cores after
   logging (about 1.5–2×): 1 / (1 − C/2) with C = 0.1–0.25 gives 1.05–1.14×. With C under 0.1 the mode is not built.

### The T4 method in steps

What to record:

1. On a PC, run the game's x86 code as it is: Windows with DXVK's i386 `d3d9.dll` dropped in, or Wine and DXVK on an
   x86 Linux. Same `.exe`, same linked D3DX, same DXVK; only the system DLLs differ, which is the least of the main
   thread.
2. Trace only the main thread with drmemtrace, offline, with instruction encodings, for a window of 60–300 frames
   in a race. Mark frames with `-record_function` on the `Present` entry of DXVK (or by its address), and record calls
   into `d3d9.dll`. At about 13 M x86 instructions a frame (41.9 M cycles at 45 fps, IPC about 0.7, 2.19 ARM64 per x86)
   that is 0.8–4 G instructions, a few GB of trace.
3. Call and return markers come from the instruction stream itself (`call`, `ret`); loop iterations from taken
   backward branches to the same target. Mark system calls and calls into each module by address range.

How to compute:

4. Decode each instruction (DynamoRIO's decoder) into register reads and writes; treat EFLAGS as groups (CF, the
   rest), the x87 stack registers by physical slot, TOP as a separate resource.
5. Keep a shadow time per register and per 4-byte memory word (the last writer's time and region). For each dynamic
   instruction: start = max(ready time of its sources and of the memory it reads); end = start + latency (1, or A57
   latencies from `[A57]`: load 4, FP 5–6, conversions 5–24). Only true dependences count; memory is renamed.
6. Run the same trace through several models, each a switch:
   - A, dataflow limit: as above, perfect control.
   - B, A plus the stack: dependences through ESP and EBP arithmetic and through dead stack slots removed (Postiff).
   - C, B plus calls into `d3d9.dll` and system DLLs serialised as one ordered resource (the side effects that must
     stay in order).
   - D, C with control dependence: an instruction cannot start before the last mispredicted branch before it
     (a simple predictor model), which is what speculation without oracle sees.
7. HCPA per region (Kremlin): each function invocation and each loop is a region; its critical path counts only
   dependences produced inside it. Self-parallelism SP(R) = (sum of the children's critical paths + R's own work) /
   critical path of R. Aggregate per static function and loop: total work share, mean SP, number of invocations,
   instructions per invocation.
8. Loop dependence distance (Alchemist): for each hot loop, the smallest iteration distance of a loop-carried true
   dependence through memory, and the share of iterations with one; classify DOALL, DOALL with rare conflicts,
   DOACROSS, serial. Reductions and induction variables are recognised and removed.
9. The four-core model (Kismet-like): the regions selected by 7–8 are scheduled on N = 2, 3 cores besides the main
   one, with a hand-off cost L per spawn and per cross-core dependence (L from C3: try 100, 300, 1000 cycles) and a
   slowdown factor S for logged code (try 1.5, 2, 3). Output: speedup of the frame against N, L, S, and which regions
   carry it.
10. The same trace through `drcachesim` with the A57's caches (32 KB 2-way L1D per core, 2 MB 16-way shared L2,
    64-byte lines) gives the L1 and L2 misses per load address for W5 and T1, and the pages written per frame for W2.

The tool: a drmemtrace analysis tool in C++ (`analysis_tool_t`, https://dynamorio.org/sec_drcachesim_newtool.html),
kept in `tools/analysis/`. A pass over 1 G instructions with a shadow map should take tens of minutes on a PC.

Cross-checks: model A on the integer parts should land near the published integer figures (10–100), and model D near
2–10; far off means the decoder or the shadow map is wrong.

## Needs the console or the lab

| Name | How | Decision it switches |
|---|---|---|
| S1 main-thread trace | drmemtrace on a PC, NFSU2 race, main thread only, 60–300 frames (the lab PC; DynamoRIO download agreed first) | input to S2–S4, W5's misses and W2's pages |
| S2 dependence limits | the analysis tool, models A–D and HCPA per function and loop | if model D at function and loop level is under 1.3× on four cores: no speculation in FEX; above 2×: S3 |
| S3 loop candidates | the dependence-distance and four-core model of steps 8–9 with L of 100–1000 cycles and S of 1.5–3 | which loops go in the per-game profile; coverage C of the gain formula; build the region mode only if C ≥ 0.1 |
| S4 calls to outside code in candidate regions | from the trace: system calls, system-DLL and `d3d9.dll` calls per invocation | whether those regions can be speculated or must stop |
| S5 hand-off cost | `[A57]`: two threads on different cores exchange a cache line (ping-pong), and one wakes the other through a futex (console; also C3) | L in the model; minimum chunk size |
| S6 logged-access cost | `[A57]`: a loop of loads and stores with and without a Janus-like log (address, value, write buffer), cycles per access (console) | S in the model |
