import { useTranslation } from "react-i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { Group } from "@/shared/api/bindings/Group";
import { CatalogField } from "./CatalogField";
import { isShown } from "./catalogValues";
import styles from "./editors.module.css";

type CatalogGroupProps = {
  catalog: Catalog;
  group: Group;
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
};

export function CatalogGroup({ catalog, group, value, onChange }: CatalogGroupProps) {
  const { t } = useTranslation();
  if (!isShown(catalog, value, group.condition)) {
    return null;
  }
  return (
    <details className={styles.advanced}>
      <summary>{t(group.name)}</summary>
      <div className={styles.stack}>
        {group.pages.map((page) => (
          <div key={page.id ?? group.id} className={styles.group}>
            {page.name ? <span className={styles.groupTitle}>{t(page.name)}</span> : null}
            <div className={styles.grid2}>
              {page.settings.map((setting) => (
                <CatalogField key={setting.id} catalog={catalog} setting={setting} value={value} onChange={onChange} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </details>
  );
}
