import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AdvancedEditor } from "@/features/autorun-settings/AdvancedEditor";
import { GraphicsEditor } from "@/features/autorun-settings/GraphicsEditor";
import { InputEditor } from "@/features/autorun-settings/InputEditor";
import { LanguageSwitch } from "@/features/preferences/LanguageSwitch";
import { usePreferences } from "@/features/preferences/usePreferences";
import { api, isCommandError } from "@/shared/api/commands";
import type { KeysReport } from "@/shared/api/bindings/KeysReport";
import type { Preferences } from "@/shared/api/bindings/Preferences";
import type { Theme } from "@/shared/api/bindings/Theme";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/Field";
import { Notice } from "@/shared/ui/Notice";
import { PageHeader } from "@/shared/ui/PageHeader";
import { Panel } from "@/shared/ui/Panel";
import { Segmented } from "@/shared/ui/Segmented";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./SettingsPage.module.css";

type DefaultsTab = "controls" | "graphics";

export function SettingsPage() {
  const { t } = useTranslation();
  const { preferences, save } = usePreferences();
  const [keys, setKeys] = useState<KeysReport | null>(null);
  const [tab, setTab] = useState<DefaultsTab>("controls");
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [artKey, setArtKey] = useState(preferences.steamGridDbKey ?? "");
  const [confirmReset, setConfirmReset] = useState(false);

  const resetDefaults = async () => {
    const recommended = await api.recommendedSettings();
    update({ defaults: recommended });
    setConfirmReset(false);
  };
  const update = (patch: Partial<Preferences>) => void save({ ...preferences, ...patch });
  const defaults = preferences.defaults;

  const pickKeys = async () => {
    const path = await open({ multiple: false, filters: [{ name: "prod.keys", extensions: ["keys"] }] });
    if (typeof path === "string") {
      update({ keysPath: path });
      setKeys(await api.checkKeys(path));
    }
  };

  const pickLibrary = async () => {
    const path = await open({ directory: true, multiple: false });
    if (typeof path !== "string") {
      return;
    }
    setLibraryError(null);
    try {
      const libraryDir = await api.libraryDirFor(path);
      await api.createLibraryDir(libraryDir);
      update({ libraryDir });
    } catch (failure) {
      setLibraryError(isCommandError(failure) ? failure.code : "unknown");
    }
  };

  return (
    <div className={styles.page}>
      <PageHeader title={t("settings.title")} backTo="/library" backLabel={t("library.title")} />
      <main className={styles.main}>
        <Panel className={`${styles.section} ${styles.appearance}`}>
          <h2>{t("settings.appearance")}</h2>
          <div className={styles.row}>
            <Field label={t("settings.language")}>
              <LanguageSwitch />
            </Field>
            <Field label={t("settings.theme")}>
              <Segmented<Theme>
                label={t("settings.theme")}
                value={preferences.theme}
                options={[
                  { value: "system", label: t("settings.system") },
                  { value: "light", label: t("settings.light") },
                  { value: "dark", label: t("settings.dark") },
                ]}
                onChange={(theme) => update({ theme })}
              />
            </Field>
          </div>
        </Panel>

        <Panel className={styles.section}>
          <h2>{t("settings.files")}</h2>
          <Field label={t("onboarding.keysPath")} htmlFor="settings-keys" hint={t("onboarding.keysPrivacy")}>
            <div className={styles.pathRow}>
              <TextInput id="settings-keys" value={preferences.keysPath ?? ""} readOnly />
              <Button onClick={() => void pickKeys()}>{t("common.browse")}</Button>
            </div>
          </Field>
          {keys && !(keys.headerKey && keys.keyAreaKey) ? <Notice tone="error">{t("settings.keysIncomplete")}</Notice> : null}
          <Field label={t("onboarding.libraryDir")} htmlFor="settings-library">
            <div className={styles.pathRow}>
              <TextInput id="settings-library" value={preferences.libraryDir ?? ""} readOnly />
              <Button onClick={() => void pickLibrary()}>{t("common.browse")}</Button>
            </div>
          </Field>
          {libraryError ? <Notice tone="error">{t(`errors.${libraryError}`)}</Notice> : null}
          <Field label={t("settings.steamGridDbKey")} htmlFor="settings-sgdb" hint={t("settings.steamGridDbHint")}>
            <TextInput
              id="settings-sgdb"
              type="password"
              autoComplete="off"
              value={artKey}
              onChange={(event) => setArtKey(event.target.value)}
              onBlur={() => {
                const steamGridDbKey = artKey.trim() === "" ? null : artKey.trim();
                if (steamGridDbKey !== preferences.steamGridDbKey) {
                  update({ steamGridDbKey });
                }
              }}
            />
          </Field>
        </Panel>

        <Panel className={styles.section}>
          <div className={styles.sectionHead}>
            <div>
              <h2>{t("settings.defaults")}</h2>
              <p className={styles.muted}>{t("settings.defaultsHint")}</p>
            </div>
            <Segmented<DefaultsTab>
              label={t("settings.defaults")}
              value={tab}
              options={[
                { value: "controls", label: t("wizard.step.controls") },
                { value: "graphics", label: t("wizard.step.graphics") },
              ]}
              onChange={setTab}
            />
          </div>
          {tab === "controls" ? (
            <InputEditor value={defaults.input} onChange={(input) => update({ defaults: { ...defaults, input } })} />
          ) : (
            <>
              <GraphicsEditor value={defaults.graphics} onChange={(graphics) => update({ defaults: { ...defaults, graphics } })} />
              <AdvancedEditor
                system={defaults.system}
                debug={defaults.debug}
                onSystemChange={(system) => update({ defaults: { ...defaults, system } })}
                onDebugChange={(debug) => update({ defaults: { ...defaults, debug } })}
              />
            </>
          )}
          {confirmReset ? (
            <Notice tone="warning">
              <p className={styles.resetText}>{t("settings.resetConfirm")}</p>
              <div className={styles.resetActions}>
                <Button variant="primary" onClick={() => void resetDefaults()}>
                  {t("settings.resetYes")}
                </Button>
                <Button variant="ghost" onClick={() => setConfirmReset(false)}>
                  {t("common.cancel")}
                </Button>
              </div>
            </Notice>
          ) : (
            <div className={styles.resetRow}>
              <Button variant="ghost" onClick={() => setConfirmReset(true)}>
                {t("settings.reset")}
              </Button>
            </div>
          )}
        </Panel>
      </main>
    </div>
  );
}
