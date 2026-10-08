import { useEffect } from "react";
import { MODIFIERS } from "./keyNames";

const CHORD_MAX = 4;

export function useKeyCapture(active: boolean, onCapture: (codes: number[]) => void) {
  useEffect(() => {
    if (!active) {
      return;
    }
    const held: number[] = [];
    const down = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      const code = event.keyCode;
      if (MODIFIERS.has(code)) {
        if (!held.includes(code) && held.length < CHORD_MAX - 1) {
          held.push(code);
        }
        return;
      }
      onCapture([...held, code]);
    };
    const up = (event: KeyboardEvent) => {
      if (MODIFIERS.has(event.keyCode) && held.length > 0) {
        event.preventDefault();
        onCapture([...held]);
      }
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    return () => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
    };
  }, [active, onCapture]);
}
