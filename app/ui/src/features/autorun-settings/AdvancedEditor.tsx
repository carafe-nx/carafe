import { useTranslation } from "react-i18next";
import type { CpuEmulator } from "@/shared/api/bindings/CpuEmulator";
import type { DebugSettings } from "@/shared/api/bindings/DebugSettings";
import type { SyncMode } from "@/shared/api/bindings/SyncMode";
import type { SystemSettings } from "@/shared/api/bindings/SystemSettings";
import type { WindowOutput } from "@/shared/api/bindings/WindowOutput";
import { Field } from "@/shared/ui/Field";
import { Segmented } from "@/shared/ui/Segmented";
import { Toggle } from "@/shared/ui/Toggle";
import styles from "./editors.module.css";

type AdvancedEditorProps = {
  system: SystemSettings;
  debug: DebugSettings;
  onSystemChange: (next: SystemSettings) => void;
  onDebugChange: (next: DebugSettings) => void;
};

export function AdvancedEditor({ system, debug, onSystemChange, onDebugChange }: AdvancedEditorProps) {
  const { t } = useTranslation();
  return (
    <details className={styles.advanced}>
      <summary>{t("advanced.title")}</summary>
      <div className={styles.stack}>
        <div className={styles.group}>
          <span className={styles.groupTitle}>{t("advanced.system")}</span>
          <Field label={t("advanced.sync")} hint={t(`advanced.syncHints.${system.sync}`)}>
            <Segmented<SyncMode>
              label={t("advanced.sync")}
              value={system.sync}
              options={[
                { value: "horizon", label: "Horizon" },
                { value: "standard", label: t("advanced.syncStandard") },
              ]}
              onChange={(sync) => onSystemChange({ ...system, sync })}
            />
          </Field>
          <Field label={t("advanced.cpu")} hint={t(`advanced.cpuHints.${system.cpu}`)}>
            <Segmented<CpuEmulator>
              label={t("advanced.cpu")}
              value={system.cpu}
              options={[
                { value: "fex", label: "FEX" },
                { value: "box64", label: "Box64" },
              ]}
              onChange={(cpu) => onSystemChange({ ...system, cpu })}
            />
          </Field>
          <Field label={t("advanced.windows")} hint={t(`advanced.windowsHints.${system.windows}`)}>
            <Segmented<WindowOutput>
              label={t("advanced.windows")}
              value={system.windows}
              options={[
                { value: "auto", label: t("advanced.windowsAuto") },
                { value: "framebuffer", label: t("advanced.windowsFramebuffer") },
                { value: "compositor", label: t("advanced.windowsCompositor") },
              ]}
              onChange={(windows) => onSystemChange({ ...system, windows })}
            />
          </Field>
          <Toggle
            id="four-cores"
            label={t("advanced.fourCores")}
            hint={t("advanced.fourCoresHint")}
            checked={system.fourCores}
            onChange={(fourCores) => onSystemChange({ ...system, fourCores })}
          />
        </div>
        <div className={styles.group}>
          <span className={styles.groupTitle}>{t("advanced.debug")}</span>
          <Toggle
            id="verbose"
            label={t("advanced.verbose")}
            hint={t("advanced.verboseHint")}
            checked={debug.verbose}
            onChange={(verbose) => onDebugChange({ ...debug, verbose })}
          />
          <Toggle
            id="profile"
            label={t("advanced.profile")}
            hint={t("advanced.profileHint")}
            checked={debug.profile}
            onChange={(profile) => onDebugChange({ ...debug, profile })}
          />
        </div>
      </div>
    </details>
  );
}
