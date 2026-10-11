import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { BuildState } from "@/shared/api/bindings/BuildState";
import type { TitleId } from "@/shared/api/bindings/TitleId";
import { api } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import { ProgressBar } from "@/shared/ui/ProgressBar";
import { Spinner } from "@/shared/ui/Spinner";
import styles from "./BuildBanner.module.css";
import { CancelBuildButton } from "./CancelBuildButton";
import { overallPercent } from "./overallPercent";

type BuildBannerProps = {
  status: BuildState | null;
  knownGames: TitleId[];
  onFinished: () => void;
};

export function BuildBanner({ status, knownGames, onFinished }: BuildBannerProps) {
  const { t } = useTranslation();
  const running = useRef(false);

  useEffect(() => {
    if (status === null) {
      return;
    }
    const now = status.state === "running";
    if (running.current && !now) {
      onFinished();
    }
    running.current = now;
  }, [status, onFinished]);

  if (status === null || status.state === "idle") {
    return null;
  }
  const close = (
    <Button variant="ghost" onClick={() => void api.dismissBuild()}>
      {t("common.close")}
    </Button>
  );

  if (status.state === "running") {
    if (status.cancelling) {
      return (
        <div className={styles.banner} role="status">
          <span className={styles.icon}>
            <Spinner />
          </span>
          <span className={styles.text}>
            <b>{t("buildTask.cancelling", { title: status.title })}</b>
            <span className={styles.muted}>{t("buildTask.cancellingHint")}</span>
          </span>
        </div>
      );
    }
    return (
      <div className={styles.banner} role="status">
        <span className={`${styles.icon} ${styles.busy}`}>
          <Icon name="rebuild" size={18} />
        </span>
        <span className={styles.text}>
          <b>
            {t("buildTask.running", { title: status.title })} · {t(`stages.${status.stage}`)}
          </b>
          <ProgressBar label={t("build.progress")} value={overallPercent(status.stage, status.percent)} />
        </span>
        <CancelBuildButton title={status.title} rebuild={knownGames.includes(status.titleId)} />
      </div>
    );
  }

  if (status.state === "done") {
    return (
      <div className={styles.banner} role="status">
        <span className={`${styles.icon} ${styles.ok}`}>
          <Icon name="check" size={18} />
        </span>
        <span className={styles.text}>
          <b>{t("buildTask.done", { title: status.title })}</b>
          <span className={styles.muted}>{t("buildTask.doneHint")}</span>
        </span>
        {close}
      </div>
    );
  }

  if (status.state === "cancelled") {
    return (
      <div className={styles.banner} role="status">
        <span className={styles.icon}>
          <Icon name="info" size={18} />
        </span>
        <span className={styles.text}>
          <b>{t("buildTask.cancelled", { title: status.title })}</b>
          <span className={styles.muted}>{t("buildTask.cancelledHint")}</span>
        </span>
        {close}
      </div>
    );
  }

  return (
    <div className={styles.banner} role="alert">
      <span className={`${styles.icon} ${styles.error}`}>
        <Icon name="alert" size={18} />
      </span>
      <span className={styles.text}>
        <b>{t("buildTask.failed", { title: status.title })}</b>
        <span className={styles.muted}>{t(`errors.${status.error.code}`)}</span>
      </span>
      {close}
    </div>
  );
}
