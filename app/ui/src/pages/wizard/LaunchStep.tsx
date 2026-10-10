import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/api/commands";
import type { Arch } from "@/shared/api/bindings/Arch";
import type { Warning } from "@/shared/api/bindings/Warning";
import type { WizardDraft } from "@/shared/api/bindings/WizardDraft";
import { formatBytes } from "@/shared/format";
import { Field } from "@/shared/ui/Field";
import { Notice } from "@/shared/ui/Notice";
import { Segmented } from "@/shared/ui/Segmented";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./WizardPage.module.css";

type LaunchStepProps = {
  draft: WizardDraft;
  executable: string | null;
  archOverride: Arch | null;
  args: string;
  onExecutable: (path: string) => void;
  onArchOverride: (arch: Arch | null) => void;
  onArgs: (args: string) => void;
};

type ArchChoice = Arch | "auto";

const ARCH_WARNINGS: readonly Warning[] = ["x86OnX64", "x64OnFixedAddress"];

function bits(arch: Arch): string {
  return arch === "x86" ? "32 bit" : "64 bit";
}

export function LaunchStep({
  draft,
  executable,
  archOverride,
  args,
  onExecutable,
  onArchOverride,
  onArgs,
}: LaunchStepProps) {
  const { t, i18n } = useTranslation();
  const [warnings, setWarnings] = useState<Warning[]>([]);
  const [archReset, setArchReset] = useState(false);
  const [moreOpen, setMoreOpen] = useState(archOverride !== null);
  const selected = draft.executables.find((choice) => choice.info.path === executable) ?? null;

  useEffect(() => {
    void api.wizardWarnings(draft, executable, archOverride).then(setWarnings);
  }, [draft, executable, archOverride]);

  function pickExecutable(path: string) {
    setArchReset(archOverride !== null && path !== executable);
    onExecutable(path);
  }

  function pickArch(choice: ArchChoice) {
    setArchReset(false);
    onArchOverride(choice === "auto" ? null : choice);
  }

  const stepWarnings = warnings.filter((warning) => !ARCH_WARNINGS.includes(warning));
  const archWarnings = warnings.filter((warning) => ARCH_WARNINGS.includes(warning));
  const fileName = selected?.info.path.split("\\").pop() ?? "";
  const archHint = selected
    ? archOverride === null
      ? t("launch.archHintAuto", { file: fileName, arch: bits(selected.info.arch) })
      : t(archOverride === "x86" ? "launch.archHintX86" : "launch.archHintX64")
    : null;

  return (
    <div className={styles.stepBody}>
      <p className={styles.lead}>{t("launch.text")}</p>
      <fieldset className={styles.choices}>
        <legend className={styles.srOnly}>{t("launch.text")}</legend>
        {draft.executables.map((choice) => {
          const checked = choice.info.path === executable;
          const manual = checked && archOverride !== null;
          return (
            <label key={choice.info.path} className={styles.choice} data-checked={checked}>
              <input type="radio" name="executable" checked={checked} onChange={() => pickExecutable(choice.info.path)} />
              <span className={styles.choiceName}>{choice.info.path}</span>
              {choice.kind === "installer" ? <span className={styles.badgeWarn}>{t("launch.installer")}</span> : null}
              <span className={manual ? styles.badgeManual : styles.badge}>
                {manual ? `${bits(archOverride)} · ${t("launch.manual")}` : bits(choice.info.arch)}
              </span>
              <span className={styles.muted}>{formatBytes(choice.info.sizeBytes, i18n.language)}</span>
            </label>
          );
        })}
      </fieldset>
      {archReset && selected ? (
        <p className={styles.muted} role="status">
          {t("launch.archWasReset", { arch: bits(selected.info.arch) })}
        </p>
      ) : null}
      {stepWarnings.map((warning) => (
        <Notice key={warning} tone="warning">
          {t(`warnings.${warning}`)}
        </Notice>
      ))}
      <Field label={t("launch.args")} htmlFor="args">
        <TextInput id="args" value={args} placeholder={t("launch.argsPlaceholder")} onChange={(event) => onArgs(event.target.value)} />
      </Field>
      {selected ? (
        <details
          className={styles.more}
          open={moreOpen}
          onToggle={(event) => setMoreOpen(event.currentTarget.open)}
        >
          <summary className={styles.moreSummary}>
            {t("launch.more")}
            {archOverride !== null ? <span className={styles.moreDot} aria-label={t("launch.manual")} /> : null}
          </summary>
          <div className={styles.moreBody}>
            <span className={styles.moreLabel}>{t("launch.arch")}</span>
            <div className={styles.archRow}>
              <Segmented<ArchChoice>
                label={t("launch.arch")}
                value={archOverride ?? "auto"}
                options={[
                  { value: "auto", label: t("launch.archAuto", { arch: bits(selected.info.arch) }) },
                  { value: "x86", label: "32 bit" },
                  { value: "x64", label: "64 bit" },
                ]}
                onChange={pickArch}
              />
              {archOverride !== null ? (
                <button type="button" className={styles.linkButton} onClick={() => pickArch("auto")}>
                  {t("launch.archReset")}
                </button>
              ) : null}
            </div>
            {archWarnings.length === 0 ? <p className={styles.muted}>{archHint}</p> : null}
            {archWarnings.map((warning) => (
              <Notice key={warning} tone="warning">
                {t(`warnings.${warning}`)}
              </Notice>
            ))}
          </div>
        </details>
      ) : null}
    </div>
  );
}
