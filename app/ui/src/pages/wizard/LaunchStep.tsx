import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/api/commands";
import type { Warning } from "@/shared/api/bindings/Warning";
import type { WizardDraft } from "@/shared/api/bindings/WizardDraft";
import { formatBytes } from "@/shared/format";
import { Field } from "@/shared/ui/Field";
import { Notice } from "@/shared/ui/Notice";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./WizardPage.module.css";

type LaunchStepProps = {
  draft: WizardDraft;
  executable: string | null;
  args: string;
  onExecutable: (path: string) => void;
  onArgs: (args: string) => void;
};

export function LaunchStep({ draft, executable, args, onExecutable, onArgs }: LaunchStepProps) {
  const { t, i18n } = useTranslation();
  const [warnings, setWarnings] = useState<Warning[]>([]);

  useEffect(() => {
    void api.wizardWarnings(draft, executable).then(setWarnings);
  }, [draft, executable]);

  return (
    <div className={styles.stepBody}>
      <p className={styles.lead}>{t("launch.text")}</p>
      <fieldset className={styles.choices}>
        <legend className={styles.srOnly}>{t("launch.text")}</legend>
        {draft.executables.map((choice) => (
          <label key={choice.info.path} className={styles.choice} data-checked={choice.info.path === executable}>
            <input
              type="radio"
              name="executable"
              checked={choice.info.path === executable}
              onChange={() => onExecutable(choice.info.path)}
            />
            <span className={styles.choiceName}>{choice.info.path}</span>
            {choice.kind === "installer" ? <span className={styles.badgeWarn}>{t("launch.installer")}</span> : null}
            <span className={styles.badge}>{choice.info.arch === "x86" ? "32 bit" : "64 bit"}</span>
            <span className={styles.muted}>{formatBytes(choice.info.sizeBytes, i18n.language)}</span>
          </label>
        ))}
      </fieldset>
      {warnings.map((warning) => (
        <Notice key={warning} tone="warning">
          {t(`warnings.${warning}`)}
        </Notice>
      ))}
      <Field label={t("launch.args")} htmlFor="args">
        <TextInput id="args" value={args} placeholder={t("launch.argsPlaceholder")} onChange={(event) => onArgs(event.target.value)} />
      </Field>
    </div>
  );
}
