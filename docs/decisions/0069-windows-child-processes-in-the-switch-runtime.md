# 69. Windows child processes run inside the Switch runtime

## Context

A Windows program can start a second executable through `CreateProcess`. Carafe
Probe needs this to start its x86 and x64 workers from one run. Switch builds
need a process creation path that works with the embedded Wine server on Horizon.

## Decision

Route `NtCreateUserProcess` through the Switch runtime. Each child gets a Windows
process identity, a guest memory context and a Windows handle table. The runtime
loads the child's executable and starts its first thread for the executable's
machine type. The parent and child run within the same Horizon process and use
Wine's embedded server for inherited handles and communication. Memory views
and asynchronous I/O are released with their owning guest process.

The process creation path applies to Windows applications in general. Windows
job-list assignment is outside the supported child process path.

## Rejected options

- A separate Horizon process would require a second loader entry point and a
  protocol to transfer startup state. The guest process path already serves the
  launch scenario and uses the existing Wine server.
- Changing Probe to run its workers inside its own process would change what it
  measures and leave other callers of `CreateProcess` without support.

## Validation

In the Probe run before integration cleanup, the x86 and x64 workers both
reached the first scenario step in Carafe Lab. That result covers startup and
does not establish the outcome of later diagnostic steps. The integration patch
series applies to the pinned Autorun revision `636913ca46a3638018b3ea56995d6852db9f0165`.
