import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { Release } from "@/shared/api/bindings/Release";
import type { UpdateStatus } from "@/shared/api/bindings/UpdateStatus";
import { api, isCommandError } from "@/shared/api/commands";
import { formatBytes } from "@/shared/format";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import { Notice } from "@/shared/ui/Notice";
import { ProgressBar } from "@/shared/ui/ProgressBar";
import { Toggle } from "@/shared/ui/Toggle";
import styles from "./UpdateButton.module.css";
import { useAppVersion } from "./useAppVersion";

type UpdatePanelProps = {
  status: UpdateStatus;
  onDone: () => void;
};

export function UpdatePanel({ status, onDone }: UpdatePanelProps) {
  const { t, i18n } = useTranslation();
  const current = useAppVersion();
  const [error, setError] = useState<string | null>(null);
  const [restarting, setRestarting] = useState(false);
  const state = status.state;
  if (state.state === "idle") {
    return null;
  }
  const release = state.release;

  const run = (action: () => Promise<void>, after?: () => void) => {
    setError(null);
    action()
      .then(after)
      .catch((failure: unknown) => {
        setRestarting(false);
        setError(isCommandError(failure) ? failure.code : "unknown");
      });
  };
  const restart = () => {
    setRestarting(true);
    run(() => api.installUpdate());
  };
  const meta = (extra?: string) =>
    [current ? t("update.current", { version: current }) : null, extra].filter(Boolean).join(" · ");
  const size = release.sizeBytes ? formatBytes(release.sizeBytes, i18n.language) : undefined;

  return (
    <div className={styles.panel} role="dialog" aria-label={t("update.title")}>
      {state.state === "available" ? (
        <>
          <Head title={t("update.availableTitle", { version: release.version })} meta={meta(size)} />
          <Details release={release} onNotes={() => run(() => api.openRelease(release.version))} />
          <div className={styles.actions}>
            <Button variant="primary" onClick={() => run(() => api.downloadUpdate())}>
              <Icon name="install" size={16} />
              {t("update.download")}
            </Button>
            <Button variant="ghost" onClick={() => run(() => api.skipUpdate(), onDone)}>
              {t("update.skip")}
            </Button>
          </div>
        </>
      ) : null}

      {state.state === "downloading" ? (
        <>
          <Head
            title={t("update.downloadingTitle", { version: release.version })}
            meta={
              state.total
                ? t("update.progress", {
                    received: formatBytes(state.received, i18n.language),
                    total: formatBytes(state.total, i18n.language),
                  })
                : formatBytes(state.received, i18n.language)
            }
          />
          <ProgressBar label={t("update.downloadingTitle", { version: release.version })} value={state.total ? (state.received / state.total) * 100 : 0} />
          <p className={styles.text}>{t("update.downloadingHint")}</p>
          <div className={styles.actions}>
            <Button variant="ghost" onClick={() => run(() => api.cancelUpdateDownload())}>
              {t("common.cancel")}
            </Button>
          </div>
        </>
      ) : null}

      {state.state === "ready" ? (
        <>
          <Head title={t("update.readyTitle", { version: release.version })} meta={meta()} />
          <Details release={release} onNotes={() => run(() => api.openRelease(release.version))} />
          {status.busy ? (
            <>
              <p className={styles.text}>{t("update.busyHint")}</p>
              <Toggle
                id="update-when-idle"
                label={t("update.whenDone")}
                checked={status.installWhenIdle}
                onChange={(enabled) => run(() => api.installUpdateWhenIdle(enabled))}
              />
            </>
          ) : (
            <p className={styles.calm}>{t(status.installOnClose ? "update.onCloseSet" : "update.readyHint")}</p>
          )}
          <div className={styles.actions}>
            <Button variant="primary" disabled={status.busy || restarting} onClick={restart}>
              <Icon name="restart" size={16} />
              {t(restarting ? "update.restarting" : "update.restartNow")}
            </Button>
            {status.busy ? null : (
              <Button variant="ghost" onClick={() => run(() => api.installUpdateOnClose(!status.installOnClose))}>
                {t(status.installOnClose ? "update.onCloseCancel" : "update.onClose")}
              </Button>
            )}
          </div>
        </>
      ) : null}

      {state.state === "failed" ? (
        <>
          <Head title={t(`update.failed.${state.step}.title`)} meta={release.version} />
          <p className={styles.text}>{t(`update.failed.${state.step}.text`, { version: current ?? "" })}</p>
          <div className={styles.actions}>
            {state.step === "verify" ? null : (
              <Button variant="primary" onClick={() => run(() => api.downloadUpdate())}>
                {t("common.retry")}
              </Button>
            )}
            <Button variant="ghost" onClick={() => run(() => api.openRelease(release.version))}>
              {t("update.fromGithub")}
            </Button>
          </div>
        </>
      ) : null}

      {error ? <Notice tone="error">{t(`errors.${error}`)}</Notice> : null}
    </div>
  );
}

function Head({ title, meta }: { title: string; meta: string }) {
  return (
    <div className={styles.head}>
      <h3>{title}</h3>
      {meta ? <span className={styles.meta}>{meta}</span> : null}
    </div>
  );
}

function Details({ release, onNotes }: { release: Release; onNotes: () => void }) {
  const { t } = useTranslation();
  return (
    <>
      {release.runtimeChanged ? (
        <p className={styles.runtime}>
          <Icon name="rebuild" size={15} />
          <span>{t("update.runtimeChanged")}</span>
        </p>
      ) : null}
      <button type="button" className={styles.link} onClick={onNotes}>
        {t("update.whatsNew")}
      </button>
    </>
  );
}
