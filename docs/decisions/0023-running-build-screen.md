# 23. The running build has its own screen

Date: 2026-10-11

Builds on [21. A build is a task](0021-build-as-a-task.md).

## Context

After leaving the wizard for the library, the build banner showed the progress and "Cancel build", but there was
no way back to the build's screen.

## Decision

- The banner has "Open" while a build runs. It leads to `/build`, a screen drawn from `build://status` alone: the
  game, the progress, the stages, "Show in the library" and "Cancel build", and afterwards the result.
- Without a build, `/build` goes back to the library.

## Rejected

- Returning to the wizard: its steps live only while it is open, so it would come back empty.
- The build log on that screen: the app does not keep it once the wizard is closed.
