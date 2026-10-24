# The settings menu on the console

How the menu over a game ([decision 33](decisions/0033-in-game-settings-menu.md)) learns what it can change, and
where it keeps what was changed. Two files: `carafe/menu.txt` in the package's RomFS, written by the app, and
`carafe/changes.txt` in the game's save data, written by the console.

## `carafe/menu.txt`

UTF-8, one record per line, fields separated by a tab. No field holds a tab or a line break. Unknown record kinds
are skipped, so a newer app can add some. Every text comes once per language, in the order of the `languages` line
([decision 35](decisions/0035-every-switch-language.md)); `×15` below is that list, shortened to three in the example.

```
carafe-menu	2
languages	ru	en	zh-Hans	ja	ko	zh-Hant	fr	fr-CA	de	it	es	es-419	nl	pt	pt-BR
api	d3d9
section	graphics	Графика	Graphics	图形	…
setting	fps	program	frame-limit	now	0	-	-	Ограничение кадров	Frame limit	帧率限制	…	<hint ru>	<hint en>	<hint zh-Hans>	…
value	-	Без ограничения	No limit	不限制	…
value	30	30	30	30	…
group	dxvk	show:d3d=0,1	Опции DXVK	DXVK options	DXVK 选项	…
page	memory	Память	Memory	内存	…
setting	dxvk-budget	program	dxvk.maxMemoryBudget	restart	0	-	-	…
value	-	Авто	Auto	自动	…
value	512	512 МиБ	512 MiB	512 MiB	…
end
preset	balanced	Баланс	Balanced	平衡	…	<hint ru>	<hint en>	<hint zh-Hans>	…
set	fex-tso	0
suggest	crash	fex-tso	1	<why ru>	<why en>	<why zh-Hans>	…
key	0x20	Пробел	Space	空格	…
pad	1	B	B	B	…
```

- `carafe-menu <version>` — the first line. A runtime that does not know the version shows no menu.
- `languages <code ×15>` — the second line: `ru en zh-Hans ja ko zh-Hant fr fr-CA de it es es-419 nl pt pt-BR`. A
  runtime whose list differs shows no menu. It keeps only the console's language and English of each text.
- `api <name>` — a Direct3D the game's executable imports: `d3d8`, `d3d9`, `d3d10`, `d3d11`, `d3d12`. No `api`
  line means the app does not know, and every option is shown.
- `section <id> <name ×15>` — a tab of the menu. The ids are `controls`, `graphics`, `directx`, `processor`,
  `system`, in this order. The menu draws `controls` itself (the console map, from `key` and `pad`); its `setting`
  lines are the row above the map.
- `setting <id> <file> <key> <apply> <default> <when> <api> <name ×15> <hint ×15>` — a row:
  - `<file>` — `program` (`<exe>.wine-nx.txt`), `keys` (`<exe>.keys.txt`) or `box64` (`<exe>.box64.txt`);
  - `<key>` — the key in that file;
  - `<apply>` — `now` (the menu applies it while the game runs) or `restart`;
  - `<default>` — the index of the value the setting has when its key is missing from the file;
  - `<when>` — `-`, `show:<setting>=<i>,<j>` (the row is hidden unless that setting has one of those value
    indices) or `on:<setting>=<i>,<j>` (the row is dimmed and cannot be changed unless …);
  - `<api>` — `-` or a comma list of Direct3D names; the row is hidden when the game's Direct3D is known and is not
    in the list.
- `value <text> <label ×15>` — a value of the setting above, in order. `-` means the key is not written.
- `group <id> <when> <name ×15>` — a row that opens a list of its own. Its rows follow, up to `end`. Inside a group,
  `page <id> <name ×15>` starts a page: the list then has a chip for each page, switched with L and R.
- `preset <id> <name ×15> <hint ×15>` and the `set <setting> <index>` lines after it — a set of values. A section with
  presets gets a "Preset" row at its top; changing a setting of a preset by hand shows it as "Custom".
- `suggest <kind> <setting> <index> <why ×15>` — what the error screen's "What to try" offers for a kind of failure:
  `crash`, `memory`, `threads`.
- `key <code> <label ×15>` and `pad <code> <label ×15>` — what a button can press, in keyboard and mouse mode
  (`key.<button>` codes) and in controller mode (`pad.<button>` codes).

## `carafe/changes.txt`

Under `sdmc:/switch/wine/`, so the file overlay keeps it in the game's save data
([decision 4](decisions/0004-sdmc-overlay.md)). Only values changed on the console are in it.

```
carafe-changes	1
program	FEX_TSOENABLED	1
keys	key.A	0x20
program	dxvk.maxMemoryBudget	-
```

- `<file> <key> <value>` — the value the key gets; `-` takes the key out of the file.

## At every start

Before Autorun reads its settings, Carafe takes `<exe>.wine-nx.txt`, `<exe>.keys.txt` and `<exe>.box64.txt` from
RomFS, applies `changes.txt` to them and writes the result to the same paths under `sdmc:/switch/wine/`, where the
overlay puts them into save data and Autorun finds them. Values nobody changed on the console come from the package,
so a game built again with a newer Carafe gets its defaults. "Recommended settings" in the menu removes
`changes.txt`.

## `carafe/registry.txt`

The game's registry entries and the menu's "Game registry" rows ([decision 34](decisions/0034-game-registry.md)), in
the package's RomFS when the game has any. UTF-8, a record per line, fields separated by a tab; an empty field is the
key's default value.

```
carafe-registry	1
mode	once
set	HKCU	Software\Studio\Game	Width	4	00050000
delete	HKCU	Software\Studio\Game	Old
deletekey	HKLM	Software\Studio\Cache
row	Language	Language of the game's text
field	HKCU	Software\Studio\Game	Language	1
option	Русский	720075000000
option	English	65006e000000
```

- `mode once|always` — the entries are written once per package or at every start.
- `set <root> <path> <name> <type> <hex>` — `<root>` is `HKCU` or `HKLM`, `<type>` the Windows type number, `<hex>` the
  value's bytes as the registry stores them: strings in UTF-16LE with their terminating zero, numbers little-endian.
- `delete <root> <path> <name>` and `deletekey <root> <path>` — remove a value, or a key with everything under it.
- `row <name> <hint>`, then `field <root> <path> <name> <type>` for each value the row writes, then
  `option <label> <hex>…` with one `<hex>` per field.

When Autorun has read the registry and before the game reads it, Carafe lays the entries over it: in `once` mode only
when `carafe/registry-applied` in save data does not hold the hash of this file, which it then writes. Then it applies
`carafe/registry-pending.txt` from save data — the changes made in the menu, in the same `set`/`delete`/`deletekey` lines
under the header `carafe-registry-pending	1` — and removes it. A trial run first keeps the values those changes replace
in `sdmc:/switch/carafe/<program id>/registry-rollback.txt`; "Bring back" on the error screen makes them the pending
changes. "Restore recommended" removes the pending changes and `registry-applied`.
