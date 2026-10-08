import type { SwitchButton } from "@/shared/api/bindings/SwitchButton";

export type Side = "left" | "right";
export type Cell = "n" | "w" | "c" | "e" | "s";
export type GlyphName = "trigger" | "shoulder" | "minus" | "plus" | "stick" | "dpad" | "abxy";

export type MapCell = {
  button: SwitchButton;
  at?: Cell;
  mark?: string;
};

export type MapRow = {
  id: string;
  side: Side;
  glyph: GlyphName;
  anchor: readonly [number, number];
  cells: readonly MapCell[];
};

export const DEVICE_VIEWBOX = "-6 -16 251 124";

export const MAP_ROWS: readonly MapRow[] = [
  { id: "zl", side: "left", glyph: "trigger", anchor: [9, -7.75], cells: [{ button: "ZL" }] },
  { id: "l", side: "left", glyph: "shoulder", anchor: [4.8, 4.6], cells: [{ button: "L" }] },
  { id: "minus", side: "left", glyph: "minus", anchor: [25.8, 8.55], cells: [{ button: "MINUS" }] },
  {
    id: "stickL",
    side: "left",
    glyph: "stick",
    anchor: [7, 28],
    cells: [
      { button: "LUP", at: "n", mark: "↑" },
      { button: "LLEFT", at: "w", mark: "←" },
      { button: "STICKL", at: "c" },
      { button: "LRIGHT", at: "e", mark: "→" },
      { button: "LDOWN", at: "s", mark: "↓" },
    ],
  },
  {
    id: "dpad",
    side: "left",
    glyph: "dpad",
    anchor: [3.8, 60],
    cells: [
      { button: "UP", at: "n", mark: "↑" },
      { button: "LEFT", at: "w", mark: "←" },
      { button: "RIGHT", at: "e", mark: "→" },
      { button: "DOWN", at: "s", mark: "↓" },
    ],
  },
  { id: "zr", side: "right", glyph: "trigger", anchor: [230, -7.75], cells: [{ button: "ZR" }] },
  { id: "r", side: "right", glyph: "shoulder", anchor: [234.2, 4.6], cells: [{ button: "R" }] },
  { id: "plus", side: "right", glyph: "plus", anchor: [213.2, 8.55], cells: [{ button: "PLUS" }] },
  {
    id: "abxy",
    side: "right",
    glyph: "abxy",
    anchor: [235.3, 28],
    cells: [
      { button: "X", at: "n", mark: "X" },
      { button: "Y", at: "w", mark: "Y" },
      { button: "A", at: "e", mark: "A" },
      { button: "B", at: "s", mark: "B" },
    ],
  },
  {
    id: "stickR",
    side: "right",
    glyph: "stick",
    anchor: [232, 60],
    cells: [
      { button: "RUP", at: "n", mark: "↑" },
      { button: "RLEFT", at: "w", mark: "←" },
      { button: "STICKR", at: "c" },
      { button: "RRIGHT", at: "e", mark: "→" },
      { button: "RDOWN", at: "s", mark: "↓" },
    ],
  },
];

export const ROW_OF_BUTTON: Readonly<Record<SwitchButton, string>> = Object.fromEntries(
  MAP_ROWS.flatMap((row) => row.cells.map((cell) => [cell.button, row.id])),
) as Record<SwitchButton, string>;
