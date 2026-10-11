import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Navigate, useNavigate } from "react-router";
import { CancelBuildButton } from "@/features/build/CancelBuildButton";
import { BUILD_STAGES, overallPercent } from "@/features/build/overallPercent";
import { useBuildStatus } from "@/features/build/useBuildStatus";
import type { TitleId } from "@/shared/api/bindings/TitleId";
import { api } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import { Notice } from "@/shared/ui/Notice";
import { PageHeader } from "@/shared/ui/PageHeader";
import { Panel } from "@/shared/ui/Panel";
import { ProgressBar } from "@/shared/ui/ProgressBar";
import { Spinner } from "@/shared/ui/Spinner";
import styles from "./BuildPage.module.css";

export function BuildPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const status = useBuildStatus();
  const [knownGames, setKnownGames] = useState<TitleId[]>([]);

  useEffect(() => {
    let active = true;
    void api.listGames().then((games) => {
      if (active) {
        setKnownGames(games.map((game) => game.titleId));
      }
    });
    return () => {
      active = false;
    };
  }, []);

  if (status === null) {
    return null;
  }
  if (status.state === "idle") {
    return <Navigate to="/library" replace />;
  }

  const running = status.state === "running";
  const stageIndex = running ? BUILD_STAGES.indexOf(status.stage) : status.state === "done" ? BUILD_STAGES.length : -1;
  const toLibrary = (
    <Button variant={running ? "ghost" : "primary"} onClick={() => navigate("/library")}>
      {running ? t("buildTask.toLibrary") : t("build.toLibrary")}
    </Button>
  );

  return (
    <div className={styles.page}>
      <PageHeader title={status.title} backTo="/library" backLabel={t("library.title")} />
      <main className={styles.main}>
        <Panel className={styles.panel}>
          {running ? (
            <>
              <h2>{t("buildTask.running", { title: status.title })}</h2>
              <ProgressBar label={t("build.progress")} value={overallPercent(status.stage, status.percent)} />
              <ol className={styles.stages}>
                {BUILD_STAGES.map((stage, index) => (
                  <li key={stage} data-state={index < stageIndex ? "done" : index === stageIndex ? "active" : "todo"}>
                    <span className={styles.stageMark}>
                      {index < stageIndex ? <Icon name="check" size={14} /> : index + 1}
                    </span>
                    {t(`stages.${stage}`)}
                  </li>
                ))}
              </ol>
              {status.cancelling ? (
                <Notice tone="info">
                  <span className={styles.noticeRow}>
                    <Spinner />
                    {t("buildTask.cancelling", { title: status.title })}
                  </span>
                </Notice>
              ) : (
                <div className={styles.actions}>
                  {toLibrary}
                  <CancelBuildButton title={status.title} rebuild={knownGames.includes(status.titleId)} />
                </div>
              )}
            </>
          ) : (
            <>
              {status.state === "done" ? <Notice tone="info">{t("buildTask.done", { title: status.title })}</Notice> : null}
              {status.state === "cancelled" ? (
                <Notice tone="info">{t("buildTask.cancelled", { title: status.title })}</Notice>
              ) : null}
              {status.state === "failed" ? <Notice tone="error">{t(`errors.${status.error.code}`)}</Notice> : null}
              <div className={styles.actions}>{toLibrary}</div>
            </>
          )}
        </Panel>
      </main>
    </div>
  );
}
