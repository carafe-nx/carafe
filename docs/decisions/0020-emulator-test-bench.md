# 20. Games are tested automatically in Eden

Date: 2026-10-10

## Context

Every change is checked on a console by hand ([testing.md](../testing.md)): build, install, launch, look, fetch logs.
It is slow and covers few games and few settings. Research ([automated-testing](../research/automated-testing/REPORT.md))
found no emulator known to run the runtime, but none ruled out by the kernel side; the GPU path and JIT speed are open.

## Decision

- The test bench emulates Horizon OS with **Eden**, run headless in Docker; a time-boxed spike comes first and decides
  how far the bench goes (section 7 of the report).
- Frames come from the PC's GPU through WSL2; software rendering stays the fallback.
- Each game runs the full scenario: launch, loading screen, input reaches the game, save survives a restart, exit and
  relaunch. Pass or fail comes from the log milestones first, screenshots compared with golden images second.
- Build settings are checked with probe programs, not games: settings files by Rust tests for every value; input and
  graphics probes in the emulator over a pairwise set of settings, plus random combinations in a nightly run.
- The result is an HTML report: games, steps, screenshots, log excerpts; what the emulator cannot show is marked
  "check on the console".
- The console stays the final check before a release.

## Rejected

- Ryubing: no scripted input or HOME, several nvmap ioctls unsupported.
- Full-system emulation (tegra_qemu, mizu): no GPU, needs boot dumps, unmaintained.
- Remote control of a real console (sys-botbase): needs a sysmodule on the user's console.
- Every combination of settings: about 250,000 runs.
