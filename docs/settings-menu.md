# The settings menu on the console

How the menu over a game ([decision 33](decisions/0033-in-game-settings-menu.md)) learns what it can change, and
where it keeps what was changed. Two files: `carafe/menu.txt` in the package's RomFS, written by the app, and
`carafe/changes.txt` in the game's save data, written by the console.

## `carafe/menu.txt`

UTF-8, one record per line, fields separated by a tab. No field holds a tab or a line break. Unknown record kinds
are skipped, so a newer app can add some.

```
carafe-menu	1
api	d3d9
section	graphics	Графика	Graphics	图形
setting	fps	program	frame-limit	now	0	-	-	Ограничение кадров	Frame limit	帧率限制	<hint ru>	<hint en>	<hint zh>
value	-	Без ограничения	No limit	不限制
value	30	30	30	30
group	dxvk	show:d3d=0,1	Опции DXVK	DXVK options	DXVK 选项
page	memory	Память	Memory	内存
setting	dxvk-budget	program	dxvk.maxMemoryBudget	restart	0	-	-	…
value	-	Авто	Auto	自动
value	512	512 МиБ	512 MiB	512 MiB
end
preset	balanced	Баланс	Balanced	平衡	<hint ru>	<hint en>	<hint zh>
set	fex-tso	0
suggest	crash	fex-tso	1	<why ru>	<why en>	<why zh>
key	0x20	Пробел	Space	空格
pad	1	B	B	B
```

- `carafe-menu <version>` — the first line. A runtime that does not know the version shows no menu.
- `api <name>` — a Direct3D the game's executable imports: `d3d8`, `d3d9`, `d3d10`, `d3d11`, `d3d12`. No `api`
  line means the app does not know, and every option is shown.
- `section <id> <ru> <en> <zh>` — a tab of the menu. The ids are `controls`, `graphics`, `directx`, `processor`,
  `system`, in this order. The menu draws `controls` itself (the console map, from `key` and `pad`); its `setting`
  lines are the row above the map.
- `setting <id> <file> <key> <apply> <default> <when> <api> <name ×3> <hint ×3>` — a row:
  - `<file>` — `program` (`<exe>.wine-nx.txt`), `keys` (`<exe>.keys.txt`) or `box64` (`<exe>.box64.txt`);
  - `<key>` — the key in that file;
  - `<apply>` — `now` (the menu applies it while the game runs) or `restart`;
  - `<default>` — the index of the value the setting has when its key is missing from the file;
  - `<when>` — `-`, `show:<setting>=<i>,<j>` (the row is hidden unless that setting has one of those value
    indices) or `on:<setting>=<i>,<j>` (the row is dimmed and cannot be changed unless …);
  - `<api>` — `-` or a comma list of Direct3D names; the row is hidden when the game's Direct3D is known and is not
    in the list.
- `value <text> <label ×3>` — a value of the setting above, in order. `-` means the key is not written.
- `group <id> <when> <name ×3>` — a row that opens a list of its own. Its rows follow, up to `end`. Inside a group,
  `page <id> <name ×3>` starts a page: the list then has a chip for each page, switched with L and R.
- `preset <id> <name ×3> <hint ×3>` and the `set <setting> <index>` lines after it — a set of values. A section with
  presets gets a "Preset" row at its top; changing a setting of a preset by hand shows it as "Custom".
- `suggest <kind> <setting> <index> <why ×3>` — what the error screen's "What to try" offers for a kind of failure:
  `crash`, `memory`, `threads`.
- `key <code> <label ×3>` and `pad <code> <label ×3>` — what a button can press, in keyboard and mouse mode
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
