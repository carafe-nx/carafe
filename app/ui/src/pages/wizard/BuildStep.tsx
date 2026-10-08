import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { renderIcon } from "@/features/icon-render/renderIcon";
import { api, isCommandError } from "@/shared/api/commands";
import type { BuildRequest } from "@/shared/api/bindings/BuildRequest";
import type { BuildStage } from "@/shared/api/bindings/BuildStage";
import { useTauriEvent } from "@/shared/api/useTauriEvent";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import { Notice } from "@/shared/ui/Notice";
import { ProgressBar } from "@/shared/ui/ProgressBar";
import styles from "./WizardPage.module.css";

const STAGES: BuildStage[] = ["runtime", "game", "metadata", "pack", "finish"];

type Run =
  | { kind: "idle" }
  | { kind: "running"; stage: BuildStage; percent: number; log: string[] }
  | { kind: "done"; log: string[] }
  | { kind: "failed"; code: string; log: string[] };

type BuildStepProps = {
  request: BuildRequest | null;
  rebuild: boolean;
};

export function BuildStep({ request, rebuild }: BuildStepProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [run, setRun] = useState<Run>({ kind: "idle" });

  useTauriEvent("build://progress", (progress) => {
    setRun((current) =>
      current.kind === "running"
        ? {
            kind: "running",
            stage: progress.stage,
            percent: progress.percent,
            log: progress.log ? [...current.log, progress.log] : current.log,
          }
        : current,
    );
  });
  useTauriEvent("build://finished", (finished) => {
    setRun((current) => {
      const log = current.kind === "idle" ? [] : current.log;
      return finished.error
        ? { kind: "failed", code: finished.error.code, log: [...log, finished.error.message] }
        : { kind: "done", log };
    });
  });

  const start = async () => {
    if (!request) {
      return;
    }
    setRun({ kind: "running", stage: "runtime", percent: 0, log: [] });
    let icon: number[];
    try {
      icon = (await renderIcon(request.metadata, request.source.folder, request.source.executable)).bytes;
    } catch (error) {
      setRun({
        kind: "failed",
        code: isCommandError(error) ? error.code : "icon",
        log: [`icon: ${isCommandError(error) ? error.message : String(error)}`],
      });
      return;
    }
    try {
      await api.startBuild({ ...request, icon });
    } catch (error) {
      setRun(
        isCommandError(error)
          ? { kind: "failed", code: error.code, log: [error.message] }
          : { kind: "failed", code: "unknown", log: [] },
      );
    }
  };

  const stageIndex = run.kind === "running" ? STAGES.indexOf(run.stage) : run.kind === "done" ? STAGES.length : -1;
  const overall = run.kind === "running" ? ((stageIndex + run.percent / 100) / STAGES.length) * 100 : run.kind === "done" ? 100 : 0;
  const log = run.kind === "idle" ? [] : run.log;

  return (
    <div className={styles.stepBody}>
      {request ? (
        <dl className={styles.summary}>
          <dt>{t("build.game")}</dt>
          <dd>{request.metadata.title}</dd>
          <dt>{t("build.executable")}</dt>
          <dd>{request.source.executable}</dd>
          <dt>{t("build.titleId")}</dt>
          <dd>{rebuild ? request.titleId : t("build.newTitleId")}</dd>
          <dt>{t("build.control")}</dt>
          <dd>{request.settings.input.mode === "controller" ? t("input.controller") : t("input.keyboardMouse")}</dd>
        </dl>
      ) : null}

      {run.kind === "idle" ? (
        <Button variant="primary" onClick={() => void start()} disabled={!request}>
          {rebuild ? t("build.startRebuild") : t("build.start")}
        </Button>
      ) : null}

      {run.kind !== "idle" ? (
        <>
          <ProgressBar label={t("build.progress")} value={overall} />
          <ol className={styles.stages}>
            {STAGES.map((stage, index) => (
              <li key={stage} data-state={index < stageIndex ? "done" : index === stageIndex ? "active" : "todo"}>
                <span className={styles.stageMark}>{index < stageIndex ? <Icon name="check" size={14} /> : index + 1}</span>
                {t(`stages.${stage}`)}
              </li>
            ))}
          </ol>
          <details className={styles.log}>
            <summary>{t("build.log")}</summary>
            <pre>{log.join("\n")}</pre>
          </details>
        </>
      ) : null}

      {run.kind === "done" ? (
        <>
          <Notice tone="info">{rebuild ? t("build.doneRebuild") : t("build.done")}</Notice>
          <Button variant="primary" onClick={() => navigate("/library")}>
            {t("build.toLibrary")}
          </Button>
        </>
      ) : null}
      {run.kind === "failed" ? (
        <>
          <Notice tone="error">{t(`errors.${run.code}`)}</Notice>
          <Button onClick={() => void start()}>{t("common.retry")}</Button>
        </>
      ) : null}
    </div>
  );
}
