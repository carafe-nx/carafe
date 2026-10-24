import { useTranslation } from "react-i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { Direct3d } from "@/shared/api/bindings/Direct3d";
import type { DxvkSource } from "@/shared/api/bindings/DxvkSource";
import type { Section } from "@/shared/api/bindings/Section";
import { Field } from "@/shared/ui/Field";
import { Select } from "@/shared/ui/Select";
import { CatalogField } from "./CatalogField";
import { CatalogGroup } from "./CatalogGroup";
import styles from "./editors.module.css";

type DirectXEditorProps = {
  catalog: Catalog;
  section: Section;
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
};

export function DirectXEditor({ catalog, section, value, onChange }: DirectXEditorProps) {
  const { t } = useTranslation();
  const graphics = value.graphics;
  const setGraphics = (patch: Partial<AutorunSettings["graphics"]>) => onChange({ ...value, graphics: { ...graphics, ...patch } });
  const sourceOptions: { value: DxvkSource; label: string }[] = [
    { value: "official", label: t("graphics.dxvkOfficial") },
    { value: "gplasync", label: "GPL Async" },
    ...(graphics.direct3d === "dxvkVkd3d" ? [] : [{ value: "sarek" as const, label: "Sarek" }]),
  ];

  return (
    <div className={styles.stack}>
      <div className={styles.grid2}>
        <Field label={t("graphics.direct3d")} htmlFor="direct3d" hint={t(`graphics.direct3dHints.${graphics.direct3d}`)}>
          <Select<Direct3d>
            id="direct3d"
            value={graphics.direct3d}
            options={[
              { value: "dxvk", label: "DXVK" },
              { value: "dxvkVkd3d", label: "DXVK + VKD3D" },
              { value: "wine", label: t("graphics.wined3d") },
            ]}
            onChange={(direct3d) =>
              setGraphics({
                direct3d,
                dxvkSource: direct3d === "dxvkVkd3d" && graphics.dxvkSource === "sarek" ? "official" : graphics.dxvkSource,
              })
            }
          />
        </Field>
        {graphics.direct3d !== "wine" ? (
          <Field label={t("graphics.dxvkSource")} htmlFor="dxvk-source" hint={t(`graphics.dxvkSourceHints.${graphics.dxvkSource}`)}>
            <Select<DxvkSource>
              id="dxvk-source"
              value={graphics.dxvkSource}
              options={sourceOptions}
              onChange={(dxvkSource) => setGraphics({ dxvkSource })}
            />
          </Field>
        ) : null}
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
