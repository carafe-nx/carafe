import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { UpdateMode } from "@/shared/api/bindings/UpdateMode";
import type { UpdateStatus } from "@/shared/api/bindings/UpdateStatus";
import { api, isCommandError } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/Field";
import { Segmented } from "@/shared/ui/Segmented";
import styles from "./UpdateSettings.module.css";
import { useUpdateStatus } from "./useUpdateStatus";

const MIN_CHECK_MS = 600;
const RESULT_MS = 6000;

type UpdateSettingsProps = {
  mode: UpdateMode;
  onModeChange: (mode: UpdateMode) => void;
};

function ago(at: number, language: string, justNow: string): string {
  const minutes = Math.round((Date.now() - at) / 60000);
  if (minutes < 1) {
    return justNow;
  }
  const format = new Intl.RelativeTimeFormat(language, { numeric: "auto" });
  if (minutes < 60) {
    return format.format(-minutes, "minute");
  }
  const hours = Math.round(minutes / 60);
  return hours < 24 ? format.format(-hours, "hour") : format.format(-Math.round(hours / 24), "day");
}

export function UpdateSettings({ mode, onModeChange }: UpdateSettingsProps) {
  const { t, i18n } = useTranslation();
  const status = useUpdateStatus();
  const [checking, setChecking] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  useEffect(() => {
    if (result === null) {
      return;
    }
    const timer = window.setTimeout(() => setResult(null), RESULT_MS);
    return () => window.clearTimeout(timer);
  }, [result]);

  const describe = (next: UpdateStatus) => {
    const release = next.state.state === "idle" ? null : next.state.release;
    return release ? t("update.found", { version: release.version }) : t("update.upToDate");
  };

  const check = async () => {
    setChecking(true);
    setResult(null);
    const started = Date.now();
    let message: string;
    try {
      message = describe(await api.checkUpdates());
    } catch (failure) {
      message = t(`errors.${isCommandError(failure) ? failure.code : "unknown"}`);
    }
    const left = MIN_CHECK_MS - (Date.now() - started);
    if (left > 0) {
      await new Promise((resolve) => window.setTimeout(resolve, left));
    }
    setChecking(false);
    setResult(message);
  };

  const checkedAt = status?.checkedAt
    ? t("update.checkedAt", { when: ago(status.checkedAt, i18n.language, t("update.justNow")) })
    : null;

  return (
    <div className={styles.updates}>
      <Field label={t("settings.updates")} hint={t(`settings.updatesHint.${mode}`)}>
        <div>
          <Segmented<UpdateMode>
            label={t("settings.updates")}
            value={mode}
            options={[
              { value: "automatic", label: t("settings.updatesMode.automatic") },
              { value: "notifyOnly", label: t("settings.updatesMode.notifyOnly") },
              { value: "off", label: t("settings.updatesMode.off") },
            ]}
            onChange={onModeChange}
          />
        </div>
      </Field>
      {status && !status.enabled ? (
        <p className={styles.muted}>{t("update.devBuild")}</p>
      ) : (
        <div className={styles.checkRow}>
          <Button disabled={checking || status === null} onClick={() => void check()}>
            {checking ? <span className={styles.spinner} aria-hidden="true" /> : null}
            {t(checking ? "update.checking" : "update.check")}
          </Button>
          <span className={styles.muted} role="status">
            {result ?? checkedAt}
          </span>
        </div>
      )}
    </div>
  );
}
