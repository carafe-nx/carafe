import { useEffect, useReducer, useState } from "react";
import { useTranslation } from "react-i18next";
import { useParams } from "react-router";
import { AdvancedEditor } from "@/features/autorun-settings/AdvancedEditor";
import { GraphicsEditor } from "@/features/autorun-settings/GraphicsEditor";
import { InputEditor } from "@/features/autorun-settings/InputEditor";
import { usePreferences } from "@/features/preferences/usePreferences";
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
  const [state, dispatch] = useReducer(wizardReducer, preferences.defaults, initialState);
  const [folderMissing, setFolderMissing] = useState(false);
  const index = STEPS.indexOf(state.step);
  const rebuild = titleId !== undefined;

  useEffect(() => {
    if (!titleId) {
      return;
    }
    void (async () => {
      const record = await api.getRecord(titleId);
      try {
        const draft = await api.inspectFolder(record.source.folder);
        dispatch({ type: "recordLoaded", record, draft });
      } catch {
        setFolderMissing(true);
        dispatch({ type: "recordLoaded", record, draft: null });
      }
    })();
  }, [titleId]);

  const next = STEPS[index + 1];
  const previous = STEPS[index - 1];

  return (
    <div className={styles.page}>
      <PageHeader
        title={rebuild ? t("wizard.rebuildTitle", { title: state.metadata.title }) : t("wizard.newTitle")}
        backTo="/library"
        backLabel={t("library.title")}
      />
      <main className={state.step === "controls" ? `${styles.main} ${styles.mainWide}` : styles.main}>
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

          {state.step === "folder" ? <FolderStep draft={state.draft} onDraft={(draft) => dispatch({ type: "draftLoaded", draft })} /> : null}
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
            />
          ) : null}
          {state.step === "controls" ? (
            <InputEditor value={state.settings.input} onChange={(input) => dispatch({ type: "settings", settings: { ...state.settings, input } })} />
          ) : null}
          {state.step === "graphics" ? (
            <div className={styles.stepBody}>
              <GraphicsEditor
                value={state.settings.graphics}
                onChange={(graphics) => dispatch({ type: "settings", settings: { ...state.settings, graphics } })}
              />
              <AdvancedEditor
                system={state.settings.system}
                debug={state.settings.debug}
                onSystemChange={(system) => dispatch({ type: "settings", settings: { ...state.settings, system } })}
                onDebugChange={(debug) => dispatch({ type: "settings", settings: { ...state.settings, debug } })}
              />
            </div>
          ) : null}
          {state.step === "build" ? <BuildStep request={toRequest(state)} rebuild={rebuild} /> : null}

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
                <Button variant="primary" disabled={!canLeave(state, state.step)} onClick={() => dispatch({ type: "goTo", step: next })}>
                  {t("common.next")}
                </Button>
              ) : null}
            </div>
          ) : null}
        </Panel>
      </main>
    </div>
  );
}
