# 32. A game that fails comes back to an error screen

Date: 2026-10-20

## Context

When a game failed, the console went back to the HOME Menu, or the game stood still: a program that ended with
an error code closed like one that ended normally, and a fault in the runtime's own code parked the thread that
took it for good. Nothing on the screen said what had happened. The look of the screen was chosen with the user
from seven drafts: the layout of draft F on draft E's wine-coloured background, the game's icon and name, two
buttons — restart and exit — and on the right the code, the module and the time played, without log lines.

## Decision

- `runtime/overlay/carafe_error.c` notes what goes wrong while the game runs: an exception no handler took (`0037`),
  a thread creation refused for good (`0038`), a fault in the runtime's own code (`0039`).
- When the program ends with exit code 3 (`abort`) or an NTSTATUS error, or the runtime itself faulted, the runtime
  on its way back to the launcher (`0036`) writes `sdmc:/switch/carafe/<program id>/carafe-error.txt` and starts
  itself again with `--carafe-error` instead of going to the menu. A user's quit, a normal exit and the components
  setup do not.
- A game that crashes with a Direct3D device open leaves pages with the GPU, and the loader cannot take the process
  back. The runtime then restarts the application (`appletRestartProgram`), and the loader, finding the report
  outside the overlay, starts the runtime with `--carafe-error`.
- Started that way, the runtime (`main` is wrapped) does not start Wine. It draws the screen into a libnx
  framebuffer: the text on the left, a card of the game on the right with its icon, name and the details (code,
  module, time played), the buttons at the bottom left. The icon and title come from the package's RomFS, the
  language from the console. Left and right choose a button, A takes the chosen one, B or + exit, a tap takes the
  tapped one; the A mark sits on the chosen button only, which is ringed in the app's accent (`#DB6E87`). Restart
  starts the package's `/carafe/argv` again, as the loader does; exit leaves no program to load, and the loader
  closes the application.
- The text is set in M PLUS 1p (Regular, Bold, ExtraBold), shipped unchanged in the runtime's RomFS under the SIL
  Open Font License 1.1 (`runtime/overlay/fonts/`, the licence in `switch/wine/licenses/`); about 5 MB per NSP.
  Without the files the console's shared font is used.
- The kind of failure picks the text: out of memory (`0xC0000017`, `0xC000012D`), missing files (`0xC0000135`,
  `0xC0000139`, `0xC000007B`, `0xC0000142`; only exit is offered), out of threads (a creation was refused), the
  runtime's own fault, any other crash.
- A fault in the runtime's own code goes on at a quit point on the thread's own stack and asks the program to
  end. A second fault, or a quit that cannot finish, parks the thread as before.

## Rejected

- Drawing the screen in the failed process: after a crash the compositor, Mesa and Wine can be in any state.
- Restarting the whole application for every failure: slower than loading the runtime again when the loader can
  take the process back.
- Buttons for logs: the user saw no use in them on the console.
- A details panel at the top right: without the log lines it was a small box over empty space.
- Buttons bound to A and B: choosing with the stick and taking the choice with A is what the console's own menus do.
- The console's shared font with synthesized bold: the headings came out rough.
