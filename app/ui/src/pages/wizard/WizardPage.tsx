import { useCallback, useEffect, useReducer, useState } from "react";
import { useTranslation } from "react-i18next";
import { useParams } from "react-router";
import { SettingsEditor } from "@/features/autorun-settings/SettingsEditor";
import { busyReason } from "@/features/build/busyReason";
import { useBuildStatus } from "@/features/build/useBuildStatus";
import { usePreferences } from "@/features/preferences/usePreferences";
import { useRebuildStatus } from "@/features/rebuild/useRebuildStatus";
import { api } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Notice } from "@/shared/ui/Notice";
import { PageHeader } from "@/shared/ui/PageHeader";
import { Panel } from "@/shared/ui/Panel";
import { BuildStep } from "./BuildStep";
import { FolderStep } from "./FolderStep";
import { LaunchStep } from "./LaunchStep";
import { LookStep } from "./LookStep";
import styles from "./WizardPage.module.css";
import { canLeave, canReach, initialState, STEPS, toRequest, wizardReducer } from "./wizardState";

export function WizardPage() {
  const { t } = useTranslation();
  const { titleId } = useParams();
  const { preferences } = usePreferences();
  const [state, dispatch] = useReducer(wizardReducer, preferences.defaults, (defaults) =>
    initialState(defaults, titleId !== undefined),
  );
  const [folderMissing, setFolderMissing] = useState(false);
  const build = useBuildStatus();
  const rebuildStatus = useRebuildStatus();
  const index = STEPS.indexOf(state.step);
  const rebuild = titleId !== undefined;

  useEffect(() => {
    if (!titleId) {
      return;
    }
    dispatch({ type: "loading", what: "folder", on: true });
    void (async () => {
      try {
        const record = await api.getRecord(titleId);
        try {
          const draft = await api.inspectFolder(record.source.folder);
          dispatch({ type: "recordLoaded", record, draft });
        } catch {
          setFolderMissing(true);
          dispatch({ type: "recordLoaded", record, draft: null });
        }
      } finally {
        dispatch({ type: "loading", what: "folder", on: false });
      }
    })();
  }, [titleId]);

  const setFolderBusy = useCallback((on: boolean) => dispatch({ type: "loading", what: "folder", on }), []);
  const setIconLoading = useCallback((on: boolean) => dispatch({ type: "loading", what: "icon", on }), []);
  const waiting = (state.step === "folder" && state.loading.folder) || (state.step === "look" && state.loading.icon);

  const next = STEPS[index + 1];
  const previous = STEPS[index - 1];

  return (
    <div className={styles.page}>
      <PageHeader
        title={rebuild ? t("wizard.rebuildTitle", { title: state.metadata.title }) : t("wizard.newTitle")}
        backTo="/library"
        backLabel={t("library.title")}
      />
      <main className={state.step === "settings" ? `${styles.main} ${styles.mainWide}` : styles.main}>
        <nav className={styles.stepper} aria-label={t("wizard.steps")}>
          {STEPS.map((step, i) => (
            <button
              key={step}
              type="button"
              className={styles.stepPill}
              data-state={step === state.step ? "current" : i <= state.reached ? "done" : "todo"}
              aria-current={step === state.step ? "step" : undefined}
              disabled={i > state.reached || step === state.step || !canReach(state, step)}
              onClick={() => dispatch({ type: "goTo", step })}
            >
              <span className={styles.stepNumber}>{i + 1}</span>
              {t(`wizard.step.${step}`)}
            </button>
          ))}
        </nav>

        <Panel className={styles.panel}>
          <h2>{t(`wizard.heading.${state.step}`)}</h2>
          {folderMissing && state.step === "folder" ? <Notice tone="warning">{t("wizard.folderMissing")}</Notice> : null}

          {state.step === "folder" ? (
            <FolderStep
              draft={state.draft}
              busy={state.loading.folder}
              onDraft={(draft) => dispatch({ type: "draftLoaded", draft })}
              onBusy={setFolderBusy}
            />
          ) : null}
          {state.step === "launch" && state.draft ? (
            <LaunchStep
              draft={state.draft}
              executable={state.executable}
              archOverride={state.archOverride}
              args={state.args}
              onExecutable={(path) => dispatch({ type: "executable", path })}
              onArchOverride={(arch) => dispatch({ type: "archOverride", arch })}
              onArgs={(args) => dispatch({ type: "args", args })}
            />
          ) : null}
          {state.step === "look" ? (
            <LookStep
              metadata={state.metadata}
              buildNumber={state.buildNumber}
              folder={state.draft?.folder ?? null}
              executable={state.executable}
              company={state.draft?.executables.find((choice) => choice.info.path === state.executable)?.info.companyName ?? null}
              onChange={(metadata) => dispatch({ type: "metadata", metadata })}
              onLoading={setIconLoading}
            />
          ) : null}
          {state.step === "settings" ? (
            <SettingsEditor
              value={state.settings}
              onChange={(settings) => dispatch({ type: "settings", settings })}
              withRegistry
            />
          ) : null}
          {state.step === "build" ? (
            <BuildStep
              request={toRequest(state)}
              rebuild={rebuild}
              busyReason={busyReason(t, build, rebuildStatus?.running ?? false)}
              cancelling={build?.state === "running" && build.cancelling}
            />
          ) : null}

          {state.step !== "build" ? (
            <div className={styles.footer}>
              {previous ? (
                <Button variant="ghost" onClick={() => dispatch({ type: "goTo", step: previous })}>
                  {t("common.back")}
                </Button>
              ) : (
                <span />
              )}
              {next ? (
                <div className={styles.footerNext}>
                  {waiting ? <span className={styles.muted}>{t("wizard.waitLoading")}</span> : null}
                  <Button variant="primary" disabled={!canLeave(state, state.step)} onClick={() => dispatch({ type: "goTo", step: next })}>
                    {t("common.next")}
                  </Button>
                </div>
              ) : null}
            </div>
          ) : null}
        </Panel>
      </main>
    </div>
  );
}
