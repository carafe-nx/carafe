import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { Binding } from "@/shared/api/bindings/Binding";
import { Button } from "@/shared/ui/Button";
import { Select } from "@/shared/ui/Select";
import styles from "./editors.module.css";
import { chordLabel, MOUSE_FIRST, mouseActions } from "./keyNames";
import { useKeyCapture } from "./useKeyCapture";

type BindingRowProps = {
  binding: Binding;
  pad: boolean;
  capturing: boolean;
  onCaptureChange: (capturing: boolean) => void;
  onChange: (codes: number[]) => void;
};

const PAD_TARGETS = Array.from({ length: 24 }, (_, index) => index + 1);

export function BindingRow({ binding, pad, capturing, onCaptureChange, onChange }: BindingRowProps) {
  const { t } = useTranslation();
  const commit = useCallback(
    (codes: number[]) => {
      onChange(codes);
      onCaptureChange(false);
    },
    [onChange, onCaptureChange],
  );
  useKeyCapture(capturing && !pad, commit);

  const label = t(`switch.${binding.button}`);
  const padOptions = [
    { value: 0, label: t("input.unbound") },
    ...PAD_TARGETS.map((target) => ({ value: target, label: t(`pad.t${target}`) })),
  ];
  const mouseOptions = [
    { value: -1, label: t("input.mousePick") },
    ...mouseActions.map((action, index) => ({ value: MOUSE_FIRST + index, label: t(`mouse.${action}`) })),
  ];

  return (
    <tr className={capturing ? styles.capturing : undefined}>
      <th scope="row">{label}</th>
      <td>
        {pad ? (
          <Select
            value={binding.codes[0] ?? 0}
            options={padOptions}
            onChange={(target) => onChange(target === 0 ? [] : [target])}
          />
        ) : capturing ? (
          <div className={styles.captureBox}>
            <span className={styles.captureText}>{t("input.pressKey")}</span>
            <Select value={-1} options={mouseOptions} onChange={(code) => code >= 0 && commit([code])} />
          </div>
        ) : (
          <span className={binding.codes.length ? undefined : styles.unbound}>{chordLabel(binding.codes, t, false)}</span>
        )}
      </td>
      <td className={styles.rowActions}>
        {pad ? null : capturing ? (
          <Button variant="ghost" onClick={() => onCaptureChange(false)}>
            {t("common.cancel")}
          </Button>
        ) : (
          <>
            <Button variant="ghost" onClick={() => onCaptureChange(true)}>
              {t("input.assign")}
            </Button>
            <Button variant="ghost" onClick={() => onChange([])} disabled={binding.codes.length === 0}>
              {t("input.clear")}
            </Button>
          </>
        )}
      </td>
    </tr>
  );
}
