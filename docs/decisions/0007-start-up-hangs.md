# 7. Start-up hangs: fixes, a safety net and diagnostics in the release

Date: 2026-10-05 – 2026-10-08

## Context

OpenTTD hung at start-up in roughly one run out of three. Thread stacks and diagnostics showed three separate problems.
Details are in [docs/autorun.md](../autorun.md), "Start-up hangs".

## Decision

- `0002-horizon-fd-owner`: messages in both descriptor queues of `horizon.c` are tagged with the request channel of
  their thread, and a thread takes its own first. Before, connections took other threads' channels during bursts of
  thread creation.
- `0003-horizon-alias-region`: Wine's search for free memory skips the alias region, where the kernel does not map
  code, instead of retrying `svcMapProcessCodeMemory` forever.
- `0005-sync-alert-wait-slices`: an endless `NtWaitForAlertByThreadId` waits 250 ms at a time. A lost wake costs a
  quarter of a second instead of the thread. The root cause of the lost wake is not found.
- The diagnostics stay in the release: `0001-horizon-hang-diagnostics`, `0004-sync-alert-diagnostics` and
  `runtime/overlay/carafe_diag.c` print `[CARAFE-DIAG]` next to Autorun's `[STALL]` report. Players send these logs with
  "Fetch logs", which is the only way to study a hang in a game nobody else can reproduce. The cost, a few atomic
  operations per wait and wake, is not measured yet.
- FEX's TSO mode is not turned on: it did not remove the hang and slowed start-up from about 37 s to 47–54 s.

## Rejected

- Making termination stronger than suspension in `select`: it treats one trace of the race while a connection is still
  served by another thread.
- Changing FEX: it is right, the thread ids differ because of `horizon.c`.
- Removing the diagnostics before the release.
