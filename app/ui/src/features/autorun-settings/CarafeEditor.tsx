import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { Section } from "@/shared/api/bindings/Section";
import { CatalogField } from "./CatalogField";
import styles from "./editors.module.css";

type CarafeEditorProps = {
  catalog: Catalog;
  section: Section;
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
};

export function CarafeEditor({ catalog, section, value, onChange }: CarafeEditorProps) {
  return (
    <div className={styles.stack}>
      <div className={styles.grid2}>
        {section.entries.map((entry) =>
          entry.kind === "setting" && entry.storage === "option" ? (
            <CatalogField key={entry.id} catalog={catalog} setting={entry} value={value} onChange={onChange} />
          ) : null,
        )}
      </div>
    </div>
  );
}
