# 34. A game's registry is set in the app and changed in the console menu

Date: 2026-10-24

## Context

Many Windows games keep their settings in the registry: resolution, language, controls, skipped intros. Guides for them
hand out `.reg` files. Carafe had no way to put a value into a game's registry, and on the console nothing could change it
without a PC. Each game already has a registry of its own: Autorun keeps `system.reg` and `user.reg` under
`sdmc:/switch/wine/registry/`, which Carafe's overlay puts into the game's save data ([research](../research/game-registry/REPORT.md)).
The look was chosen with the user on the "Registry" page of the menu canvas.

## Decision

- **In the app**, the build wizard's "Settings" step gets "Registry" under "System". It holds:
  - entries — set a value, delete a value, delete a key — imported from a `.reg` file or added by hand, shown in a
    table and edited in place, and exported back to a `.reg` file;
  - rows for the console menu: a name, a hint, the values the row writes together and its choices, each giving every
    value a text;
  - when the entries are written: **once** (at the first start and after the game is built with other entries; what the
    game or the player changes later stays) or **at every start**. Once is the default.
  The settings for new games do not have the section: entries belong to one game.
- **Import** reads what Windows' and Wine's `regedit` read: UTF-16LE with a byte order mark, UTF-8, otherwise
  Windows-1251; `REGEDIT4` and version 5 headers; strings, `dword:`, `hex:` and `hex(N):`, continued lines, deletions.
  `HKEY_CLASSES_ROOT` becomes `HKLM\Software\Classes` and `HKLM\Software\Wow6432Node` becomes `HKLM\Software`, because
  the console's registry has one view for 32- and 64-bit programs. Keys under roots the game's registry does not have
  (`HKEY_USERS`, `HKEY_CURRENT_CONFIG`) and lines that cannot be read are listed, not imported. The player ticks what to
  take; an entry for the same value replaces the old one.
- **The package** carries `carafe/registry.txt`: the mode, the entries with their data as hex bytes, and the menu rows
  ([format](../settings-menu.md#caraferegistrytxt)).
- **On the console**, Autorun calls Carafe when the registry has been read, before the game sees it (patch 0049). Carafe
  lays the package's entries over it — once per package, or at every start — then the changes made in the menu.
- **In the menu**, "System" gets "Game registry" above "Restore recommended". It opens with two views switched with L and
  R:
  - "From the app": the rows prepared in the app, each showing the choice that matches the registry now, or "Own value";
  - "Whole registry": `HKEY_CURRENT_USER` and `HKEY_LOCAL_MACHINE`, keys opened with A and left with B, values with their
    type and data. A edits a number with a number pad (decimal or hexadecimal, Y switches) or a string with an on-screen
    keyboard; X creates a value; − pressed twice deletes it; Y takes a change back. Other types are shown, not edited.
- **When changes take effect**: every registry change waits for a restart and is listed in the restart dialog. Changes
  are kept in the save data and applied at the next start, before the game reads the registry; the live registry is not
  changed while the game runs, since a game holding its settings in memory would write the old ones back on exit.
- **Trial run**: when a game started after registry changes fails, "Bring back" restores the values from before them.
  "Restore recommended" drops the changes waiting for a restart and writes the package's entries again at the next start.
- **Autorun** gets one patch, `0049`, to `horizon_registry_server.h`: the hook `carafeRegistryLoaded` at the end of
  `horizon_registry_init`, and `wine_nx_registry_*` calls that walk, read and change the tree under the server's lock.
- Texts are in Russian, English and Simplified Chinese.

## Rejected

- Writing the hive files in Carafe's start-up constructor: a second parser and writer of Wine's registry format, and a race
  with the server's own writes.
- Changing the live registry from the menu: the game may write its copy back and undo the change.
- Running Wine's `reg import` before the game: a second program start under FEX costs seconds at every launch.
- Applying the package's entries at every start by default: it would undo what the game itself saves in the registry.
- A sixth menu section: "System" already holds the rows about the whole game; the user agreed to five sections.
- Editing binary and multi-string values on the console: rare in game settings and unreadable with a gamepad; the app
  edits them.

## Open

- Not verified on a console; checked in Carafe Lab with a 32-bit registry probe.
- Typing on the console has Latin letters, digits and symbols only.
