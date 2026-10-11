import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, isCommandError } from "@/shared/api/commands";
import type { WizardDraft } from "@/shared/api/bindings/WizardDraft";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import { Notice } from "@/shared/ui/Notice";
import { Spinner } from "@/shared/ui/Spinner";
import styles from "./WizardPage.module.css";

type FolderStepProps = {
  draft: WizardDraft | null;
  busy: boolean;
  onDraft: (draft: WizardDraft) => void;
  onBusy: (busy: boolean) => void;
};

export function FolderStep({ draft, busy, onDraft, onBusy }: FolderStepProps) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);

  const pick = async () => {
    const folder = await open({ directory: true, multiple: false });
    if (typeof folder !== "string") {
      return;
    }
    onBusy(true);
    setError(null);
    try {
      onDraft(await api.inspectFolder(folder));
    } catch (failure) {
      setError(isCommandError(failure) ? failure.code : "unknown");
    } finally {
      onBusy(false);
    }
  };

  return (
    <div className={styles.stepBody}>
      <p className={styles.lead}>{t("folder.text")}</p>
      <button type="button" className={styles.dropZone} onClick={() => void pick()} disabled={busy} aria-busy={busy}>
        {busy ? <Spinner size="large" /> : <Icon name="folder" size={32} />}
        <span className={styles.dropTitle}>{busy ? t("folder.reading") : draft ? draft.folder : t("folder.pick")}</span>
        <span className={styles.muted}>
          {busy
            ? t("folder.readingHint")
            : draft
              ? t("folder.found", { count: draft.executables.length })
              : t("folder.pickHint")}
        </span>
      </button>
      {draft && !busy ? (
        <Button variant="ghost" onClick={() => void pick()}>
          {t("folder.change")}
        </Button>
      ) : null}
      {error ? <Notice tone="error">{t(`errors.${error}`)}</Notice> : null}
      <Notice tone="info">{t("folder.installedHint")}</Notice>
    </div>
  );
}
