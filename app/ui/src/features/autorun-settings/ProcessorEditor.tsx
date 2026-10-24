import { useTranslation } from "react-i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { CpuEmulator } from "@/shared/api/bindings/CpuEmulator";
import type { Section } from "@/shared/api/bindings/Section";
import { Field } from "@/shared/ui/Field";
import { Segmented } from "@/shared/ui/Segmented";
import { CatalogField } from "./CatalogField";
import { CatalogGroup } from "./CatalogGroup";
import { currentPreset, withPreset } from "./catalogValues";
import styles from "./editors.module.css";

const CUSTOM = "custom";

type ProcessorEditorProps = {
  catalog: Catalog;
  section: Section;
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
};

export function ProcessorEditor({ catalog, section, value, onChange }: ProcessorEditorProps) {
  const { t } = useTranslation();
  const fex = value.system.cpu === "fex";
  const preset = currentPreset(catalog, value, section.presets);

  return (
    <div className={styles.stack}>
      <Field label={t("advanced.cpu")} hint={t(`advanced.cpuHints.${value.system.cpu}`)}>
        <Segmented<CpuEmulator>
          label={t("advanced.cpu")}
          value={value.system.cpu}
          options={[
            { value: "fex", label: "FEX" },
            { value: "box64", label: "Box64" },
          ]}
          onChange={(cpu) => onChange({ ...value, system: { ...value.system, cpu } })}
        />
      </Field>
      {fex && section.presets.length > 0 ? (
        <Field label={t("catalog.ui.preset")} hint={preset ? t(preset.hint) : t("catalog.ui.presetCustom")}>
          <Segmented<string>
            label={t("catalog.ui.preset")}
            value={preset?.id ?? CUSTOM}
            options={[
              ...section.presets.map((item) => ({ value: item.id, label: t(item.name) })),
              { value: CUSTOM, label: t("catalog.presets.custom") },
            ]}
            onChange={(id) => {
              const picked = section.presets.find((item) => item.id === id);
              if (picked) {
                onChange(withPreset(catalog, value, picked));
              }
            }}
          />
        </Field>
      ) : null}
      <div className={styles.grid2}>
        {section.entries.map((entry) =>
          entry.kind === "setting" && entry.storage === "option" ? (
            <CatalogField key={entry.id} catalog={catalog} setting={entry} value={value} onChange={onChange} />
          ) : null,
        )}
      </div>
      {section.entries.map((entry) =>
        entry.kind === "group" ? <CatalogGroup key={entry.id} catalog={catalog} group={entry} value={value} onChange={onChange} /> : null,
      )}
    </div>
  );
}
