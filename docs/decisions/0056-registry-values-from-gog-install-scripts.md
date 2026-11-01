# 56. Registry values from GOG's install scripts go into the package and come back when missing

Date: 2026-11-01

## Context

A game from GOG comes with `goggame-<id>.info` and, when it needs more than its files, `goggame-<id>.script` in the
root of its folder. After copying the files, GOG's installer or GOG Galaxy runs the script with `scriptinterpreter.exe`:
its `setRegistry` steps write registry values, `{app}` standing for the game folder. A game folder copied from a PC
does not carry them. Spore from GOG reads its data folder from `HKLM\SOFTWARE\Electronic Arts\SPORE` and on the
console stops with "Could not find the default preferences. The data directory is missing or corrupt. [1004]".

Decision 34 already lays the package's registry entries over the game's registry: once per package, or at every start.

## Decision

- **At build time** the app reads every `goggame-*.script` in the root of the game folder and turns its `setRegistry`
  steps into installed values: `{app}` is `C:\Games\<folder>`, the folder as the game sees it; `{productId}` the
  script's product; `{userDocs}`, `{userAppData}` and `{localAppData}` the console user's folders. Roots, `Wow6432Node`
  and `HKEY_CLASSES_ROOT` are mapped as for `.reg` files. A step without a value name creates its key. A step with an
  unknown placeholder, root or type is left out; a script that is not JSON is skipped.
- They go into `carafe/registry.txt` as `restore` and `restorekey` lines after the player's own entries. An entry of
  the player's for the same value, or a deletion of its key, takes the installed value's place.
- **On the console** `restore` lines are written like `set` lines in the package's mode. At every start, after the
  changes made in the menu, each installed value or key that is missing is written again; one that exists, whatever
  its data, is left alone.
- Values and keys the player deletes in the console menu are kept in `carafe/registry-removed.txt` in save data and are
  not written again; setting the value in the menu takes it off the list. "Restore recommended" clears the list.

## Rejected

- Reading the script on the console at every start: a JSON parser in the runtime's start-up, and the values would be
  hidden from the package.
- Running GOG's `scriptinterpreter.exe` under Wine: a second program under FEX at the first start, and it also copies
  files and installs redistributables a game folder already has.
- Overwriting installed values at every start: a game or player that changes one would lose the change.

## Consequences

- Spore from GOG and other GOG games that keep their paths in the registry start from a copied folder.
- A game rebuilt after a menu deletion gets the installed value back once, as after a reinstall.
- Other steps of GOG's scripts (copying support files, redistributables, save paths) are not run.
