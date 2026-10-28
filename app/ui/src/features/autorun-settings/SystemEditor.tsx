import { useTranslation } from "react-i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { Section } from "@/shared/api/bindings/Section";
import { Field } from "@/shared/ui/Field";
import { Segmented } from "@/shared/ui/Segmented";
import { Toggle } from "@/shared/ui/Toggle";
import { CatalogGroup } from "./CatalogGroup";
import { currentPreset, withPreset } from "./catalogValues";
import styles from "./editors.module.css";

const CUSTOM = "custom";

type SystemEditorProps = {
  catalog: Catalog;
  section: Section;
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
};

export function SystemEditor({ catalog, section, value, onChange }: SystemEditorProps) {
  const { t } = useTranslation();
  const preset = currentPreset(catalog, value, section.presets);
  const debug = value.debug;

  return (
    <div className={styles.stack}>
      {section.entries.map((entry) =>
        entry.kind === "group" ? <CatalogGroup key={entry.id} catalog={catalog} group={entry} value={value} onChange={onChange} /> : null,
      )}
      {section.presets.length > 0 ? (
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
      <div className={styles.group}>
        <span className={styles.groupTitle}>{t("advanced.debug")}</span>
        <Toggle
          id="verbose"
          label={t("advanced.verbose")}
          hint={t("advanced.verboseHint")}
          checked={debug.verbose}
          onChange={(verbose) => onChange({ ...value, debug: { ...debug, verbose } })}
        />
        <Toggle
          id="profile"
          label={t("advanced.profile")}
          hint={t("advanced.profileHint")}
          checked={debug.profile}
          onChange={(profile) => onChange({ ...value, debug: { ...debug, profile } })}
        />
      </div>
    </div>
  );
}
