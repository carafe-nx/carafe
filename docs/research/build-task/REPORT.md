# A build as a task

Date: 2026-10-11

## Question

A build started from the wizard lived only in the wizard's screen: it could be started twice, the stages of two
builds mixed in one progress view, and it could not be cancelled.

## Findings

### Carafe

- `start_build` (`app/crates/carafe-desktop/src/commands/wizard.rs`) spawns the build and forgets it: nothing records
  that a build runs, and its progress goes out as global `build://progress` and `build://finished` events without a
  Title ID. Two calls both start; two new games get different Title IDs, so both run and their stages mix.
- The rebuild queue (`rebuilds.rs`) is the only state of a running build. It refuses a second rebuild, but not a
  wizard build, and both can collide on one game's work folder (`.carafe/build/<Title ID>`), guarded only by a file
  lock with a check-then-lock window (`carafe-pack/src/lib.rs`, `pack`).
- hacBrewPack runs as a child process whose handle is local to `tool::run`; nothing can stop it from outside.
- A failed build already cleans up: the work folder is removed after every build, and `write_nsp` removes its
  `.nsp.part`. A cancel that surfaces as an error reuses that path.

### Other apps

- Steam and Heroic show a running download on the game's tile and in a downloads view; a second start opens the
  existing job ([Steam](https://www.recharge.com/en/ca/steam/infos/how-to-cancel-a-download-on-steam),
  [Heroic FAQ](https://heroicgameslauncher.com/faq)).
- Closing a view must not cancel the job: Lutris and GitHub Desktop had bugs where it did, or where cancel had no
  effect ([Lutris #1913](https://github.com/lutris/lutris/issues/1913),
  [GitHub Desktop #2276](https://github.com/desktop/desktop/issues/2276)).
- Heroic and Lutris ask before cancelling and say what happens to partial files; a cancel that arrives while the
  job finishes, or that takes time to stop, needs its own state
  ([Heroic #2682](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2682),
  [Heroic #5606](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/5606)).
- Queues (HandBrake, DaVinci Resolve) are more than one build at a time needs.
