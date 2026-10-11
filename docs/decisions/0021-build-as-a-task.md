# 21. A build is a task: one at a time, cancellable, shown in the library

Date: 2026-10-11

Research: [build-task](../research/build-task/REPORT.md).

## Context

A wizard build was tied to the wizard's screen: it could start twice, two builds mixed their stages, and nothing
could stop one. The rebuild queue had its own state and did not see wizard builds.

## Decision

- One build runs at a time, from the wizard or the rebuild queue. While one runs, "New game" and "Rebuild" in the
  library are disabled with the reason; a wizard already open cannot start its build and says why.
- The running build is kept in the app's state (`builds.rs`) and published as `build://status`. The library shows
  it as a banner above the shelf, as the rebuild queue does: the game, the stage, the progress, "Cancel build", and
  afterwards the result until it is closed. Leaving the wizard does not stop the build.
- "Cancel build" asks first. A cancel flag (`CancelToken` in the core) is checked between files and stages, and
  hacBrewPack is killed. The cancel ends the build with the `cancelled` error, so the existing cleanup removes the
  work folder and the partial NSP; a previous NSP of the game stays. "Cancelling…" is shown until the build has
  stopped; a build that finishes before it notices the cancel stays built.

## Rejected

- Progress on the game's tile: a new game has no tile yet, and an error does not fit on it.
- Cancelling without asking: a stray click would cost the whole build.
- A queue of builds: one build at a time covers the need; a queue must be shown, edited and cancelled.
