import type { TFunction } from "i18next";

export const MOUSE_FIRST = 0x100;

export const mouseActions = [
  "left",
  "right",
  "middle",
  "back",
  "forward",
  "wheelUp",
  "wheelDown",
  "wheelLeft",
  "wheelRight",
  "up",
  "down",
  "moveLeft",
  "moveRight",
] as const;

export const MODIFIERS = new Set([0x10, 0x11, 0x12]);

const named: Record<number, string> = {
  0x08: "Backspace",
  0x09: "Tab",
  0x0d: "Enter",
  0x10: "Shift",
  0x11: "Ctrl",
  0x12: "Alt",
  0x13: "Pause",
  0x14: "Caps Lock",
  0x1b: "Esc",
  0x20: "Space",
  0x21: "Page Up",
  0x22: "Page Down",
  0x23: "End",
  0x24: "Home",
  0x25: "←",
  0x26: "↑",
  0x27: "→",
  0x28: "↓",
  0x2d: "Insert",
  0x2e: "Delete",
  0x5b: "Win",
  0x6a: "Num *",
  0x6b: "Num +",
  0x6d: "Num −",
  0x6e: "Num .",
  0x6f: "Num /",
  0x90: "Num Lock",
  0x91: "Scroll Lock",
  0xba: ";",
  0xbb: "=",
  0xbc: ",",
  0xbd: "-",
  0xbe: ".",
  0xbf: "/",
  0xc0: "`",
  0xdb: "[",
  0xdc: "\\",
  0xdd: "]",
  0xde: "'",
};

export function keyLabel(code: number, t: TFunction): string {
  if (code >= MOUSE_FIRST) {
    const action = mouseActions[code - MOUSE_FIRST];
    return action ? t(`mouse.${action}`) : `0x${code.toString(16)}`;
  }
  const fixed = named[code];
  if (fixed) {
    return fixed;
  }
  if ((code >= 0x30 && code <= 0x39) || (code >= 0x41 && code <= 0x5a)) {
    return String.fromCharCode(code);
  }
  if (code >= 0x60 && code <= 0x69) {
    return `Num ${code - 0x60}`;
  }
  if (code >= 0x70 && code <= 0x87) {
    return `F${code - 0x6f}`;
  }
  return `0x${code.toString(16)}`;
}

export function shortChordLabel(codes: readonly number[], t: TFunction, pad: boolean): string {
  if (codes.length === 0) {
    return t("input.unbound");
  }
  if (pad) {
    return codes.map((code) => t(`padShort.t${code}`)).join(" + ");
  }
  return codes
    .map((code) => {
      const action = code >= MOUSE_FIRST ? mouseActions[code - MOUSE_FIRST] : undefined;
      return action ? t(`mouseShort.${action}`) : keyLabel(code, t);
    })
    .join(" + ");
}

export function chordLabel(codes: readonly number[], t: TFunction, pad: boolean): string {
  if (codes.length === 0) {
    return t("input.unbound");
  }
  if (pad) {
    return codes.map((code) => t(`pad.t${code}`)).join(" + ");
  }
  return codes.map((code) => keyLabel(code, t)).join(" + ");
}
