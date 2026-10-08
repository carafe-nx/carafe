import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { Trans, useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { LanguageSwitch } from "@/features/preferences/LanguageSwitch";
import { ThemeSwitch } from "@/features/preferences/ThemeSwitch";
import { usePreferences } from "@/features/preferences/usePreferences";
import { api, isCommandError } from "@/shared/api/commands";
import type { KeysReport } from "@/shared/api/bindings/KeysReport";
import { Button } from "@/shared/ui/Button";
import { CarafeMark } from "@/shared/ui/CarafeMark";
import { Field } from "@/shared/ui/Field";
import { Notice } from "@/shared/ui/Notice";
import { isWindows } from "@/shared/platform";
import { Panel } from "@/shared/ui/Panel";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./OnboardingPage.module.css";

const STEPS = ["welcome", "keys", "library", "notice"] as const;
type Step = (typeof STEPS)[number];

export function OnboardingPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { preferences, save } = usePreferences();
  const [step, setStep] = useState<Step>("welcome");
  const [keysPath, setKeysPath] = useState(preferences.keysPath ?? "");
  const [keys, setKeys] = useState<KeysReport | null>(null);
  const [libraryDir, setLibraryDir] = useState(preferences.libraryDir ?? "");
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [understood, setUnderstood] = useState(false);
  const index = STEPS.indexOf(step);

  const pickKeys = async () => {
    const path = await open({ multiple: false, filters: [{ name: "prod.keys", extensions: ["keys"] }] });
    if (typeof path === "string") {
      setKeysPath(path);
      setKeys(await api.checkKeys(path));
    }
  };

  const pickLibrary = async () => {
    const path = await open({ directory: true, multiple: false });
    if (typeof path === "string") {
      setLibraryError(null);
      setLibraryDir(await api.libraryDirFor(path));
    }
  };

  const createLibrary = async () => {
    try {
      await api.createLibraryDir(libraryDir);
      return true;
    } catch (failure) {
      setLibraryError(isCommandError(failure) ? failure.code : "unknown");
      return false;
    }
  };

  const canContinue =
    step === "welcome" ||
    (step === "keys" && keys !== null && keys.headerKey && keys.keyAreaKey) ||
    (step === "library" && libraryDir.trim() !== "") ||
    (step === "notice" && understood);

  const next = async () => {
    if (step === "library" && !(await createLibrary())) {
      return;
    }
    const following = STEPS[index + 1];
    if (following) {
      setStep(following);
      return;
    }
    await save({ ...preferences, onboarded: true, keysPath, libraryDir });
    navigate("/library", { replace: true });
  };

  return (
    <div className={styles.page}>
      <div className={styles.topBar}>
        <LanguageSwitch />
        <ThemeSwitch />
      </div>
      <Panel className={styles.panel}>
        <ol className={styles.dots} aria-label={t("onboarding.progress", { step: index + 1, total: STEPS.length })}>
          {STEPS.map((item, i) => (
            <li key={item} className={i <= index ? styles.dotOn : styles.dot} />
          ))}
        </ol>

        {step === "welcome" ? (
          <div className={styles.content}>
            <CarafeMark size={64} />
            <h1>{t("onboarding.welcomeTitle")}</h1>
            <p>{t("onboarding.welcomeText")}</p>
            <p className={styles.muted}>{t("onboarding.welcomeSetup")}</p>
          </div>
        ) : null}

        {step === "keys" ? (
          <div className={styles.content}>
            <h1>{t("onboarding.keysTitle")}</h1>
            <p>{t("onboarding.keysText")}</p>
            <ol className={styles.steps}>
              <li>{t("onboarding.keysStep1")}</li>
              <li>
                <Trans i18nKey="onboarding.keysStep2" components={{ code: <code /> }} />
              </li>
              <li>{t("onboarding.keysStep3")}</li>
            </ol>
            <p className={styles.muted}>
              <Trans i18nKey="onboarding.keysNoLockpick" components={{ code: <code /> }} />
            </p>
            <Field label={t("onboarding.keysPath")} htmlFor="keys-path">
              <div className={styles.pathRow}>
                <TextInput id="keys-path" value={keysPath} readOnly placeholder="prod.keys" />
                <Button onClick={() => void pickKeys()}>{t("common.browse")}</Button>
              </div>
            </Field>
            {keys ? (
              keys.headerKey && keys.keyAreaKey ? (
                <Notice tone="info">{t("onboarding.keysOk")}</Notice>
              ) : (
                <Notice tone="error">
                  {t("onboarding.keysMissing", {
                    keys: [keys.headerKey ? null : "header_key", keys.keyAreaKey ? null : "key_area_key_application_00"]
                      .filter(Boolean)
                      .join(", "),
                  })}
                </Notice>
              )
            ) : null}
            <p className={styles.muted}>{t("onboarding.keysPrivacy")}</p>
          </div>
        ) : null}

        {step === "library" ? (
          <div className={styles.content}>
            <h1>{t("onboarding.libraryTitle")}</h1>
            <p>{t("onboarding.libraryText")}</p>
            <Field label={t("onboarding.libraryDir")} htmlFor="library-dir">
              <div className={styles.pathRow}>
                <TextInput id="library-dir" value={libraryDir} readOnly />
                <Button onClick={() => void pickLibrary()}>{t("common.browse")}</Button>
              </div>
            </Field>
            {libraryError ? <Notice tone="error">{t(`errors.${libraryError}`)}</Notice> : null}
          </div>
        ) : null}

        {step === "notice" ? (
          <div className={styles.content}>
            <h1>{t("onboarding.noticeTitle")}</h1>
            <p>{t("onboarding.noticeSource")}</p>
            {isWindows ? <p>{t("onboarding.noticeDefender")}</p> : null}
            <p>{t("onboarding.noticeWarranty")}</p>
            <label className={styles.check}>
              <input type="checkbox" checked={understood} onChange={(event) => setUnderstood(event.target.checked)} />
              {t("onboarding.noticeAccept")}
            </label>
          </div>
        ) : null}

        <div className={styles.footer}>
          {index > 0 ? (
            <Button variant="ghost" onClick={() => setStep(STEPS[index - 1] ?? "welcome")}>
              {t("common.back")}
            </Button>
          ) : (
            <span />
          )}
          <Button variant="primary" disabled={!canContinue} onClick={() => void next()}>
            {index === STEPS.length - 1 ? t("onboarding.finish") : t("common.next")}
          </Button>
        </div>
      </Panel>
    </div>
  );
}
