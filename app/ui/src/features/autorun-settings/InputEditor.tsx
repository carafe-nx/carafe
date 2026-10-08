import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { InputMode } from "@/shared/api/bindings/InputMode";
import type { InputSettings } from "@/shared/api/bindings/InputSettings";
import { Field } from "@/shared/ui/Field";
import { Segmented } from "@/shared/ui/Segmented";
import { Toggle } from "@/shared/ui/Toggle";
import { BindingRow } from "./BindingRow";
import { ControllerMap } from "./ControllerMap";
import styles from "./editors.module.css";

type View = "map" | "list";

type InputEditorProps = {
  value: InputSettings;
  onChange: (next: InputSettings) => void;
};

export function InputEditor({ value, onChange }: InputEditorProps) {
  const { t } = useTranslation();
  const [capturing, setCapturing] = useState<string | null>(null);
  const [view, setView] = useState<View>("map");
  const pad = value.mode === "controller";
  const layout = pad ? value.pad : value.keys;

  const setCodes = (index: number, codes: number[]) => {
    const next = layout.map((binding, i) => (i === index ? { ...binding, codes } : binding));
    onChange(pad ? { ...value, pad: next } : { ...value, keys: next });
  };

  return (
    <div className={styles.stack}>
      <Field label={t("input.mode")} hint={pad ? t("input.modeControllerHint") : t("input.modeKeyboardHint")}>
        <Segmented<InputMode>
          label={t("input.mode")}
          value={value.mode}
          options={[
            { value: "controller", label: t("input.controller") },
            { value: "keyboardMouse", label: t("input.keyboardMouse") },
          ]}
          onChange={(mode) => onChange({ ...value, mode })}
        />
      </Field>
      <div className={styles.grid2}>
        <Field label={t("input.leftDeadzone", { value: value.leftDeadzone })} htmlFor="left-deadzone">
          <input
            id="left-deadzone"
            className={styles.range}
            type="range"
            min={0}
            max={25}
            value={value.leftDeadzone}
            onChange={(event) => onChange({ ...value, leftDeadzone: Number(event.target.value) })}
          />
        </Field>
        <Field label={t("input.rightDeadzone", { value: value.rightDeadzone })} htmlFor="right-deadzone">
          <input
            id="right-deadzone"
            className={styles.range}
            type="range"
            min={0}
            max={25}
            value={value.rightDeadzone}
            onChange={(event) => onChange({ ...value, rightDeadzone: Number(event.target.value) })}
          />
        </Field>
        <p className={styles.hint}>{t("input.deadzoneHint")}</p>
      </div>
      <Toggle
        id="keyboard-auto"
        label={t("input.keyboardAuto")}
        hint={t("input.keyboardAutoHint")}
        checked={value.keyboardAuto}
        onChange={(keyboardAuto) => onChange({ ...value, keyboardAuto })}
      />
      <div className={styles.viewHead}>
        <Segmented<View>
          label={t("controls.view")}
          value={view}
          options={[
            { value: "map", label: t("controls.viewMap") },
            { value: "list", label: t("controls.viewList") },
          ]}
          onChange={setView}
        />
      </div>
      {view === "map" ? (
        <ControllerMap
          bindings={layout}
          pad={pad}
          onChange={(button, codes) => setCodes(layout.findIndex((binding) => binding.button === button), codes)}
        />
      ) : (
        <div className={styles.tableWrap}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th scope="col">{t("input.switchButton")}</th>
                <th scope="col">{pad ? t("input.padTarget") : t("input.keyTarget")}</th>
                <th scope="col" aria-label={t("input.actions")} />
              </tr>
            </thead>
            <tbody>
              {layout.map((binding, index) => (
                <BindingRow
                  key={binding.button}
                  binding={binding}
                  pad={pad}
                  capturing={capturing === binding.button}
                  onCaptureChange={(on) => setCapturing(on ? binding.button : null)}
                  onChange={(codes) => setCodes(index, codes)}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
