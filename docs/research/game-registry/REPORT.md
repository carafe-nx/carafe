# The game's Windows registry

Date: 2026-10-24. Where a game's registry lives on the console, how Carafe can lay entries over it before the game reads
them, how the settings menu can read and change it, and what a `.reg` file holds. Autorun's files are at `636913ca`; Wine's
own `.reg` importer is in the same tree. The decision is [34](../../decisions/0034-game-registry.md).

## Where the registry lives

- Autorun has no wineserver: the registry is part of the in-process server in `dlls/ntdll/unix/horizon.c`. It includes
  `horizon_registry.h` (`:65`), the global `horizon_registry` (`:3377`) and its lock `horizon_server_objects_mutex`
  (`:3380`), and `horizon_registry_server.h` (`:14586`). Every `horizon_reg_*` function is `static`; outside the file only
  `horizon_registry_flush` and `horizon_registry_flush_key` are visible.
- There is no thread of its own. Requests come in on the per-connection server threads
  (`horizon_server_handle_registry`, `horizon_registry_server.h:427`) under the objects lock.
- `horizon_registry_init` (`horizon_registry_server.h:278`) builds the tree lazily, at the first registry request: the base
  keys, machine and user seeds, `horizon-dlls/classes.reg`, then `registry/system.reg` and `registry/user.reg`
  (`:335-338`). The `changed` callback, which marks a hive dirty, is set last (`:343`).
- The hives are text in Wine's `WINE REGISTRY Version 2` format under `HORIZON_REGISTRY_DIR`, which is
  `sdmc:/switch/wine/` (`horizon_runtime_paths.h:4-5`, `horizon_registry_paths.h:17`). Carafe's file overlay sends
  `/switch/wine/*` to the game's device save data (`runtime/overlay/carafe_overlay.c:290-318`), so each game has its own
  registry, kept with its saves.
- Changes are written by `horizon_registry_flush` (`:195`): the text of a dirty hive is made under the lock and written to
  `.tmp`, then renamed. It runs every second from the runtime's log flusher (`horizon-wine/source/runtime.c:297-300`), at
  exit and on `RegFlushKey`.
- The tree has a single view: `KEY_WOW64_*` is not read by the open and create handlers (`:456-492`), and there is no
  `Wow6432Node` redirection (`horizon_registry.h:15-17`). A 32-bit game reads `HKLM\Software\X` where Windows would give it
  `HKLM\Software\Wow6432Node\X`.

## Laying entries over it

- `horizon_reg_load` (`horizon_registry.h:1150`) merges a `WINE REGISTRY` text over a key: a later value wins. It cannot
  delete: a line starting with `-` only counts as an error (`:1183-1185`), `"x"=-` leaves an empty `REG_NONE`. Carafe's
  own entries therefore go through `horizon_reg_set_value`, `horizon_reg_delete_value` and `horizon_reg_delete`.
- The latest safe moment is the end of `horizon_registry_init`: the hives are loaded, the `changed` callback is set, so a
  change made there marks the hive dirty and is flushed a second later; the game has not read anything yet, because the
  init runs inside its first request.
- Carafe's settings are merged earlier, in a constructor (`carafe_settings.c:433-485`), before `main`. Rewriting the hive
  text there would need a second parser and writer for Wine's format; a hook in the init reuses Autorun's.

## Reading and changing it from the menu

- The menu runs inside `wine_nx_pointer_poll`, on `wine_nx_input_thread`, which has no TEB, so `Nt*Key` calls are not
  allowed there (`dlls/win32u/winnx_drv.c:86-100`). It has to call into the server's tree directly, under the objects lock.
- A value changed in the live tree would be flushed, but the game may already hold its own copy and write it back on exit.
  Changes made on the console are therefore kept aside and applied by the same hook at the next start, before the game
  reads them.

## The `.reg` file

Ground truth is Wine's importer, `programs/reg/import.c` (and `programs/regedit/regproc.c`, almost the same).

- Encoding comes from the first two bytes: `FF FE` is UTF-16LE, anything else is 8-bit text in the ANSI code page
  (`import.c:1008-1012`). A UTF-8 byte order mark breaks Wine's header check. Line ends may be CRLF, LF or CR.
- The header is `Windows Registry Editor Version 5.00` or `REGEDIT4` (`import.c:437-461`); it does not choose the
  encoding. Windows' regedit exports v5 as UTF-16LE with a byte order mark and `REGEDIT4` as ANSI.
- `[KEY]` opens a key and creates it; `[-KEY]` deletes a tree; a line not starting with `[`, `@` or `"` is ignored
  (`import.c:538-612`). Roots: Wine's `reg` takes `HKLM`, `HKCU`, `HKCR`, `HKU`, `HKCC` and the long names; Windows' regedit
  only the long names.
- Values: `"name"="text"` with `\\`, `\"`, `\n`, `\r` escapes; `@` is the default value; `dword:` takes 1–8 hex digits;
  `hex:` is binary; `hex(N):` is type `N` in hex — `2` expandable string, `7` multi-string, `b` QWORD. Text in `hex(1/2/7)`
  follows the file's encoding. Hex data may continue on the next line after a trailing `\`. `"name"=-` deletes the value
  (`import.c:185-339`, `:647-832`).
- WOW64: on Windows `HKLM\SOFTWARE` is redirected for 32-bit programs to `HKLM\SOFTWARE\Wow6432Node`, while `HKCU\Software`
  is shared ([Registry keys affected by WOW64](https://learn.microsoft.com/en-us/windows/win32/winprog64/shared-registry-keys),
  [Registry redirector](https://learn.microsoft.com/en-us/windows/win32/winprog64/registry-redirector)). Files exported on
  a 64-bit PC for a 32-bit game therefore carry `Wow6432Node`, which the console's single view does not have.
- `HKEY_CLASSES_ROOT` is `HKLM\Software\Classes` in Wine (`dlls/kernelbase/registry.c:50-59`).

## Checked in Carafe Lab

The registry probe (`carafe-lab/tools/probes/registry.c`, 32-bit) reads `HKCU\Software\CarafeProbe` and
`HKLM\Software\CarafeProbe`, logs what it finds and counts its starts in `Starts`. Built by the app from a UTF-16 `.reg`
with a `Wow6432Node` key, an `HKEY_USERS` key and a deletion, with one menu row:

- at the first start the probe read every entry of the package, `HKLM\Software\CarafeProbe\InstallPath` included, and no
  deleted value;
- in the menu the row showed the live value; the row, an edit with the number pad, a new value and a deletion became four
  changes waiting for a restart, listed in the restart dialog;
- after the restart from the menu the probe read the new values, did not find the deleted one, and its own `Starts`
  went to 2: what the game wrote survived and the package's entries, in the "once" mode, did not overwrite the menu's.

Presses the emulator holds for 0.15 s are sometimes lost by the menu, which reads buttons when the game polls input; the
scenario holds them for 0.35 s. Not verified on a console.
