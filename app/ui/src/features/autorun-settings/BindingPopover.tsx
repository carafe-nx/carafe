import { type Ref, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { Binding } from "@/shared/api/bindings/Binding";
import styles from "./ControllerMap.module.css";
import { MOUSE_FIRST, mouseActions } from "./keyNames";
import { useKeyCapture } from "./useKeyCapture";

type BindingPopoverProps = {
  binding: Binding;
  pad: boolean;
  left: number;
  top: number;
  onChange: (codes: number[]) => void;
  ref?: Ref<HTMLDivElement>;
};

const PAD_TARGETS = Array.from({ length: 24 }, (_, index) => index + 1);

export function BindingPopover({ binding, pad, left, top, onChange, ref }: BindingPopoverProps) {
  const { t } = useTranslation();
  const commit = useCallback((codes: number[]) => onChange(codes), [onChange]);
  useKeyCapture(!pad, commit);
  const current = binding.codes.length === 1 ? binding.codes[0] : undefined;

  return (
    <div ref={ref} className={styles.popover} style={{ left, top }} role="menu" aria-label={t(`switch.${binding.button}`)}>
      <p className={styles.popoverTitle}>{t(`switch.${binding.button}`)}</p>
      {pad ? null : <p className={styles.popoverHint}>{t("controls.pressOrPick")}</p>}
      <div className={styles.popoverList}>
        {pad
          ? PAD_TARGETS.map((target) => (
              <button
                key={target}
                type="button"
                role="menuitemradio"
                aria-checked={current === target}
                className={styles.popoverItem}
                onClick={() => onChange([target])}
              >
                {t(`pad.t${target}`)}
              </button>
            ))
          : mouseActions.map((action, index) => (
              <button
                key={action}
                type="button"
                role="menuitemradio"
                aria-checked={current === MOUSE_FIRST + index}
                className={styles.popoverItem}
                onClick={() => onChange([MOUSE_FIRST + index])}
              >
                {t(`mouse.${action}`)}
              </button>
            ))}
      </div>
      <button
        type="button"
        className={styles.popoverClear}
        disabled={binding.codes.length === 0}
        onClick={() => onChange([])}
      >
        {t("input.clear")}
      </button>
    </div>
  );
}
