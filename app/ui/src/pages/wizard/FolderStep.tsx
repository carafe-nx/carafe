import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, isCommandError } from "@/shared/api/commands";
import type { WizardDraft } from "@/shared/api/bindings/WizardDraft";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import { Notice } from "@/shared/ui/Notice";
import styles from "./WizardPage.module.css";

type FolderStepProps = {
  draft: WizardDraft | null;
  onDraft: (draft: WizardDraft) => void;
};

export function FolderStep({ draft, onDraft }: FolderStepProps) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pick = async () => {
    const folder = await open({ directory: true, multiple: false });
    if (typeof folder !== "string") {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      onDraft(await api.inspectFolder(folder));
    } catch (failure) {
      setError(isCommandError(failure) ? failure.code : "unknown");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.stepBody}>
      <p className={styles.lead}>{t("folder.text")}</p>
      <button type="button" className={styles.dropZone} onClick={() => void pick()} disabled={busy}>
        <Icon name="folder" size={32} />
        <span className={styles.dropTitle}>{draft ? draft.folder : t("folder.pick")}</span>
        <span className={styles.muted}>
          {draft ? t("folder.found", { count: draft.executables.length }) : t("folder.pickHint")}
        </span>
      </button>
      {draft ? (
        <Button variant="ghost" onClick={() => void pick()}>
          {t("folder.change")}
        </Button>
      ) : null}
      {error ? <Notice tone="error">{t(`errors.${error}`)}</Notice> : null}
      <Notice tone="info">{t("folder.installedHint")}</Notice>
    </div>
  );
}
