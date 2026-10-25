import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { SectionId } from "@/shared/api/bindings/SectionId";
import { DirectXEditor } from "./DirectXEditor";
import { GraphicsEditor } from "./GraphicsEditor";
import { InputEditor } from "./InputEditor";
import { ProcessorEditor } from "./ProcessorEditor";
import { RegistryEditor } from "./RegistryEditor";
import { SystemEditor } from "./SystemEditor";
import styles from "./SettingsEditor.module.css";
import { useSettingsCatalog } from "./useSettingsCatalog";

type SettingsEditorProps = {
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
  withRegistry?: boolean;
};

export function SettingsEditor({ value, onChange, withRegistry = false }: SettingsEditorProps) {
  const { t } = useTranslation();
  const catalog = useSettingsCatalog();
  const [current, setCurrent] = useState<SectionId>("controls");
  const section = catalog?.sections.find((item) => item.id === current);
  const registryCount = value.registry.entries.length + value.registry.menu.length;

  return (
    <div className={styles.editor}>
      <nav className={styles.nav} aria-label={t("catalog.ui.sections")}>
        {(catalog?.sections ?? []).map((item) => (
          <button
            key={item.id}
            type="button"
            className={styles.navItem}
            aria-current={item.id === current ? "page" : undefined}
            onClick={() => setCurrent(item.id)}
          >
            {t(item.name)}
          </button>
        ))}
      </nav>
      <section className={styles.content} aria-label={section ? t(section.name) : undefined}>
        {current === "controls" ? (
          <InputEditor value={value.input} onChange={(input) => onChange({ ...value, input })} />
        ) : null}
        {current === "graphics" ? (
          <GraphicsEditor value={value.graphics} onChange={(graphics) => onChange({ ...value, graphics })} />
        ) : null}
        {current === "directx" && catalog && section ? (
          <DirectXEditor catalog={catalog} section={section} value={value} onChange={onChange} />
        ) : null}
        {current === "processor" && catalog && section ? (
          <ProcessorEditor catalog={catalog} section={section} value={value} onChange={onChange} />
        ) : null}
        {current === "system" ? (
          <div className={styles.system}>
            <SystemEditor
              system={value.system}
              debug={value.debug}
              onSystemChange={(system) => onChange({ ...value, system })}
              onDebugChange={(debug) => onChange({ ...value, debug })}
            />
            {withRegistry ? (
              <div className={styles.registry}>
                <span className={styles.registryTitle}>
                  {t("registry.nav")}
                  {registryCount > 0 ? <span className={styles.registryCount}>{registryCount}</span> : null}
                </span>
                <RegistryEditor value={value.registry} onChange={(registry) => onChange({ ...value, registry })} />
              </div>
            ) : null}
          </div>
        ) : null}
      </section>
    </div>
  );
}
