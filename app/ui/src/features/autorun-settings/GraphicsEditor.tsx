import { useTranslation } from "react-i18next";
import type { Direct3d } from "@/shared/api/bindings/Direct3d";
import type { DxvkHud } from "@/shared/api/bindings/DxvkHud";
import type { DxvkSource } from "@/shared/api/bindings/DxvkSource";
import type { GraphicsSettings } from "@/shared/api/bindings/GraphicsSettings";
import type { LsfgFlow } from "@/shared/api/bindings/LsfgFlow";
import type { Upscaling } from "@/shared/api/bindings/Upscaling";
import { Field } from "@/shared/ui/Field";
import { Segmented } from "@/shared/ui/Segmented";
import { Select } from "@/shared/ui/Select";
import { Toggle } from "@/shared/ui/Toggle";
import styles from "./editors.module.css";

const FRAME_LIMITS = [0, 30, 40, 45, 60, 75, 90, 120] as const;

type GraphicsEditorProps = {
  value: GraphicsSettings;
  onChange: (next: GraphicsSettings) => void;
};

export function GraphicsEditor({ value, onChange }: GraphicsEditorProps) {
  const { t } = useTranslation();
  const set = (patch: Partial<GraphicsSettings>) => onChange({ ...value, ...patch });
  const sourceOptions: { value: DxvkSource; label: string }[] = [
    { value: "official", label: t("graphics.dxvkOfficial") },
    { value: "gplasync", label: "GPL Async" },
    ...(value.direct3d === "dxvkVkd3d" ? [] : [{ value: "sarek" as const, label: "Sarek" }]),
  ];

  return (
    <div className={styles.stack}>
      <div className={styles.grid2}>
        <Field label={t("graphics.direct3d")} htmlFor="direct3d" hint={t(`graphics.direct3dHints.${value.direct3d}`)}>
          <Select<Direct3d>
            id="direct3d"
            value={value.direct3d}
            options={[
              { value: "dxvk", label: "DXVK" },
              { value: "dxvkVkd3d", label: "DXVK + VKD3D" },
              { value: "wine", label: t("graphics.wined3d") },
            ]}
            onChange={(direct3d) =>
              set({ direct3d, dxvkSource: direct3d === "dxvkVkd3d" && value.dxvkSource === "sarek" ? "official" : value.dxvkSource })
            }
          />
        </Field>
        <Field label={t("graphics.dxvkSource")} htmlFor="dxvk-source" hint={t(`graphics.dxvkSourceHints.${value.dxvkSource}`)}>
          <Select<DxvkSource> id="dxvk-source" value={value.dxvkSource} options={sourceOptions} onChange={(dxvkSource) => set({ dxvkSource })} />
        </Field>
        <Field label={t("graphics.frameLimit")} htmlFor="frame-limit" hint={t("graphics.frameLimitHint")}>
          <Select<number>
            id="frame-limit"
            value={value.frameLimit}
            options={FRAME_LIMITS.map((limit) => ({ value: limit, label: limit === 0 ? t("graphics.noLimit") : `${limit}` }))}
            onChange={(frameLimit) => set({ frameLimit })}
          />
        </Field>
        <Field label={t("graphics.hud")} htmlFor="hud" hint={t("graphics.hudHint")}>
          <Select<DxvkHud>
            id="hud"
            value={value.hud}
            options={[
              { value: "off", label: t("graphics.hudOff") },
              { value: "fps", label: t("graphics.hudFps") },
              { value: "compact", label: t("graphics.hudCompact") },
              { value: "full", label: t("graphics.hudFull") },
            ]}
            onChange={(hud) => set({ hud })}
          />
        </Field>
      </div>
      <Field label={t("graphics.upscaling")} hint={`${t(`graphics.upscalingHints.${value.upscaling}`)} ${t("graphics.upscalingScope")}`}>
        <Segmented<Upscaling>
          label={t("graphics.upscaling")}
          value={value.upscaling}
          options={[
            { value: "off", label: t("graphics.upscalingOff") },
            { value: "fsr", label: "FSR 1.0" },
            { value: "integer", label: t("graphics.upscalingInteger") },
          ]}
          onChange={(upscaling) => set({ upscaling })}
        />
      </Field>
      {value.upscaling === "fsr" ? (
        <Field label={t("graphics.sharpness", { value: value.sharpness })} htmlFor="sharpness" hint={t("graphics.sharpnessHint")}>
          <input
            id="sharpness"
            className={styles.range}
            type="range"
            min={0}
            max={100}
            step={20}
            value={value.sharpness}
            onChange={(event) => set({ sharpness: Number(event.target.value) })}
          />
        </Field>
      ) : null}
      <Toggle id="vsync" label={t("graphics.vsync")} hint={t("graphics.vsyncHint")} checked={value.vsync} onChange={(vsync) => set({ vsync })} />
      <Toggle id="lsfg" label={t("graphics.lsfg")} hint={t("graphics.lsfgHint")} checked={value.lsfg} onChange={(lsfg) => set({ lsfg })} />
      {value.lsfg ? (
        <div className={styles.grid2}>
          <Toggle
            id="lsfg-performance"
            label={t("graphics.lsfgPerformance")}
            hint={t("graphics.lsfgPerformanceHint")}
            checked={value.lsfgPerformance}
            onChange={(lsfgPerformance) => set({ lsfgPerformance })}
          />
          <Field label={t("graphics.lsfgFlow")} htmlFor="lsfg-flow" hint={t("graphics.lsfgFlowHint")}>
            <Select<LsfgFlow>
              id="lsfg-flow"
              value={value.lsfgFlow}
              options={[
                { value: "eighth", label: "12,5 %" },
                { value: "quarter", label: "25 %" },
                { value: "half", label: "50 %" },
              ]}
              onChange={(lsfgFlow) => set({ lsfgFlow })}
            />
          </Field>
        </div>
      ) : null}
    </div>
  );
}
