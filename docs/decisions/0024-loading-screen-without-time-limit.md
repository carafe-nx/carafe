# 24. The loading screen stays until the game's first frame

Date: 2026-10-12

Replaces the 180 s limit of [decision 9](0009-loading-screen.md).

## Context

The loading screen left at the latest 180 s after start, whether or not the game had drawn anything. A start that is
slow but alive then showed a black screen with no progress: in the emulator bench OpenTTD needs more than 180 s, and
the limit turned a slow start into a failure. A stalled start already has its own sign: the hint after a time without
progress.

## Decision

- The loading screen leaves only when the game's first frame is seen (the rules of decision 9) or when the program
  opens the keyboard. There is no time limit.
- After 60 s without progress (nothing read, no new frames) the loading screen shows that the game may not respond and
  that HOME closes it, and the runtime writes the stall report with the threads to the log. Both were at 30 s.

## Rejected

- A longer limit, such as 600 s: still turns a slow start into a black screen, only later.
- A separate limit for the bench: the console and the bench would show different behaviour.
