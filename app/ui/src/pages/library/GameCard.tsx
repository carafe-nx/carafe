import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { api, isCommandError } from "@/shared/api/commands";
import type { GameSummary } from "@/shared/api/bindings/GameSummary";
import type { InstallTarget } from "@/shared/api/bindings/InstallTarget";
import { useTauriEvent } from "@/shared/api/useTauriEvent";
import { formatBytes } from "@/shared/format";
import { Button } from "@/shared/ui/Button";
import { GameArt } from "@/shared/ui/GameArt";
import { Icon } from "@/shared/ui/Icon";
import { Notice } from "@/shared/ui/Notice";
import { Panel } from "@/shared/ui/Panel";
import { ProgressBar } from "@/shared/ui/ProgressBar";
import styles from "./LibraryPage.module.css";

type Install = { kind: "idle" } | { kind: "running"; percent: number } | { kind: "done" } | { kind: "failed"; failure: Failure };

type Failure = { code: string; message: string };

function failureOf(error: unknown): Failure {
  return isCommandError(error) ? { code: error.code, message: error.message } : { code: "unknown", message: String(error) };
}

type GameCardProps = {
  game: GameSummary;
  connected: boolean;
  onDeleted: () => void;
};

export function GameCard({ game, connected, onDeleted }: GameCardProps) {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const [install, setInstall] = useState<Install>({ kind: "idle" });
  const [target, setTarget] = useState<InstallTarget>("sd");
  const [logs, setLogs] = useState<{ busy: boolean; path?: string; failed?: Failure }>({ busy: false });
  const [confirmDelete, setConfirmDelete] = useState(false);

  useTauriEvent("install://progress", (event) => {
    if (event.titleId === game.titleId && event.progress) {
      setInstall({ kind: "running", percent: (event.progress.doneBytes / event.progress.totalBytes) * 100 });
    }
  });
  useTauriEvent("install://finished", (event) => {
    if (event.titleId === game.titleId) {
      setInstall(event.error ? { kind: "failed", failure: event.error } : { kind: "done" });
    }
  });

  const startInstall = async (where: InstallTarget) => {
    setTarget(where);
    setInstall({ kind: "running", percent: 0 });
    try {
      await api.installGame(game.titleId, where);
    } catch (error) {
      setInstall({ kind: "failed", failure: failureOf(error) });
    }
  };

  const fetchLogs = async () => {
    setLogs({ busy: true });
    try {
      setLogs({ busy: false, path: await api.fetchLogs(game.titleId) });
    } catch (error) {
      setLogs({ busy: false, failed: failureOf(error) });
    }
  };

  const installing = install.kind === "running";

  const remove = async () => {
    await api.deleteGame(game.titleId);
    onDeleted();
  };

  return (
    <Panel className={styles.card} aria-label={game.title}>
      <div className={styles.cardHead}>
        <GameArt title={game.title} icon={game.icon} size="card" />
        <div className={styles.cardTitle}>
          <h2>{game.title}</h2>
          <p className={styles.muted}>{game.publisher}</p>
        </div>
      </div>
      <dl className={styles.facts}>
        <dt>{t("card.version")}</dt>
        <dd>{game.version}</dd>
        <dt>{t("card.arch")}</dt>
        <dd>
          {game.arch === "x86" ? "x86 · 32 bit" : "x64 · 64 bit"}
          {game.archManual ? ` · ${t("card.archManual")}` : null}
        </dd>
        <dt>{t("card.size")}</dt>
        <dd>{formatBytes(game.sizeBytes, i18n.language)}</dd>
        <dt>Title ID</dt>
        <dd className={styles.mono}>{game.titleId}</dd>
      </dl>
      {game.runtimeOutdated ? (
        <Notice tone="info">{t("card.runtimeOutdated", { version: game.runtimeVersion })}</Notice>
      ) : null}

      <div className={styles.installBlock}>
        <span className={styles.installLabel}>{t("card.install")}</span>
        <div className={styles.actions}>
          <Button variant="primary" disabled={!connected || installing} onClick={() => void startInstall("sd")}>
            <Icon name="install" size={16} />
            {t("card.installSd")}
          </Button>
          <Button disabled={!connected || installing} onClick={() => void startInstall("nand")}>
            {t("card.installNand")}
          </Button>
        </div>
      </div>

      <div className={styles.actions}>
        <Button onClick={() => navigate(`/rebuild/${game.titleId}`)}>
          <Icon name="rebuild" size={16} />
          {t("card.rebuild")}
        </Button>
        <Button disabled={!connected || logs.busy} onClick={() => void fetchLogs()}>
          <Icon name="logs" size={16} />
          {t("card.logs")}
        </Button>
      </div>
      {!connected ? <p className={styles.muted}>{t("card.needDevice")}</p> : null}

      {install.kind === "running" ? (
        <div className={styles.progressBlock}>
          <div className={styles.progressRow}>
            <span>{t("card.installing")}</span>
            <span>{Math.round(install.percent)} %</span>
          </div>
          <ProgressBar label={t("card.installing")} value={install.percent} />
        </div>
      ) : null}
      {install.kind === "done" ? (
        <Notice tone="info">{t(target === "sd" ? "card.installedSd" : "card.installedNand")}</Notice>
      ) : null}
      {install.kind === "failed" ? (
        <Notice tone="error">
          {t(`errors.${install.failure.code}`)}
          <span className={styles.errorDetail}>{install.failure.message}</span>
        </Notice>
      ) : null}
      {logs.path ? <Notice tone="info">{t("card.logsSaved", { path: logs.path })}</Notice> : null}
      {logs.failed ? (
        <Notice tone="error">
          {t("card.logsFailed")} {t(`errors.${logs.failed.code}`)}
          <span className={styles.errorDetail}>{logs.failed.message}</span>
        </Notice>
      ) : null}

      <div className={styles.deleteZone}>
        {confirmDelete ? (
          <>
            <p>{t("card.deleteConfirm", { title: game.title })}</p>
            <div className={styles.actions}>
              <Button variant="danger" onClick={() => void remove()}>
                {t("card.deleteYes")}
              </Button>
              <Button variant="ghost" onClick={() => setConfirmDelete(false)}>
                {t("common.cancel")}
              </Button>
            </div>
          </>
        ) : (
          <Button variant="danger" onClick={() => setConfirmDelete(true)}>
            <Icon name="trash" size={16} />
            {t("card.delete")}
          </Button>
        )}
      </div>
    </Panel>
  );
}
