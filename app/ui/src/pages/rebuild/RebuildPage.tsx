import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { useRebuildStatus } from "@/features/rebuild/useRebuildStatus";
import type { GameSummary } from "@/shared/api/bindings/GameSummary";
import type { RebuildItem } from "@/shared/api/bindings/RebuildItem";
import type { TitleId } from "@/shared/api/bindings/TitleId";
import { api, isCommandError } from "@/shared/api/commands";
import { formatBytes } from "@/shared/format";
import { Button } from "@/shared/ui/Button";
import { GameArt } from "@/shared/ui/GameArt";
import { Notice } from "@/shared/ui/Notice";
import { PageHeader } from "@/shared/ui/PageHeader";
import { Panel } from "@/shared/ui/Panel";
import { ProgressBar } from "@/shared/ui/ProgressBar";
import styles from "./RebuildPage.module.css";

export function RebuildPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const status = useRebuildStatus();
  const [library, setLibrary] = useState<GameSummary[] | null>(null);
  const [runtime, setRuntime] = useState<string | null>(null);
  const [selected, setSelected] = useState<TitleId[]>([]);
  const [space, setSpace] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [started, setStarted] = useState(false);

  useEffect(() => {
    let active = true;
    void Promise.all([api.listGames(), api.rebuildOffer()]).then(([games, offer]) => {
      if (active) {
        setLibrary(games);
        setRuntime(offer.runtimeVersion);
        setSelected(offer.games);
      }
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    void api.rebuildSpaceNeeded(selected).then((bytes) => {
      if (active) {
        setSpace(bytes);
      }
    });
    return () => {
      active = false;
    };
  }, [selected]);

  const byId = useMemo(() => new Map((library ?? []).map((game) => [game.titleId, game])), [library]);
  const outdated = (library ?? []).filter((game) => game.runtimeOutdated);
  const showQueue = status !== null && (status.running || (started && status.items.length > 0));

  const toggle = (titleId: TitleId, checked: boolean) =>
    setSelected((current) => (checked ? [...current, titleId] : current.filter((id) => id !== titleId)));

  const start = () => {
    setError(null);
    const ordered = outdated.map((game) => game.titleId).filter((id) => selected.includes(id));
    api
      .startRebuild(ordered)
      .then(() => setStarted(true))
      .catch((failure: unknown) => setError(isCommandError(failure) ? failure.code : "unknown"));
  };

  return (
    <div className={styles.page}>
      <PageHeader title={t("rebuild.pageTitle")} backTo="/library" backLabel={t("library.title")} />
      <main className={styles.main}>
        {showQueue && status ? (
          <Panel className={styles.section}>
            <div className={styles.head}>
              <h2>{t(status.running ? "rebuild.queueTitle" : "rebuild.queueDone")}</h2>
              <p className={styles.muted}>{t("rebuild.reinstallHint")}</p>
            </div>
            <ul className={styles.list}>
              {status.items.map((item) => (
                <QueueRow key={item.titleId} item={item} icon={byId.get(item.titleId)?.icon ?? null} />
              ))}
            </ul>
            <div className={styles.footer}>
              {status.running ? (
                <Button variant="ghost" disabled={status.stopping} onClick={() => void api.stopRebuild()}>
                  {t(status.stopping ? "rebuild.stoppingAfter" : "rebuild.stopAfter")}
                </Button>
              ) : (
                <>
                  <span className={styles.muted}>
                    {t("rebuild.summary", {
                      done: status.items.filter((item) => item.state.state === "done").length,
                      total: status.items.length,
                    })}
                  </span>
                  <Button variant="primary" onClick={() => void navigate("/library")}>
                    {t("build.toLibrary")}
                  </Button>
                </>
              )}
            </div>
          </Panel>
        ) : (
          <Panel className={styles.section}>
            <div className={styles.head}>
              <h2>{runtime ? t("rebuild.title", { version: runtime }) : t("rebuild.pageTitle")}</h2>
              <p className={styles.muted}>{t("rebuild.intro")}</p>
            </div>
            {library !== null && outdated.length === 0 ? (
              <p className={styles.muted}>{t("rebuild.nothing")}</p>
            ) : (
              <ul className={styles.list}>
                {outdated.map((game) => (
                  <li key={game.titleId}>
                    <label className={styles.pick}>
                      <input
                        type="checkbox"
                        checked={selected.includes(game.titleId)}
                        onChange={(event) => toggle(game.titleId, event.target.checked)}
                      />
                      <span className={styles.art}>
                        <GameArt title={game.title} icon={game.icon} />
                      </span>
                      <span className={styles.name}>
                        <b>{game.title}</b>
                        <span>{t("rebuild.builtWith", { version: game.runtimeVersion })}</span>
                      </span>
                      <span className={styles.size}>{formatBytes(game.sizeBytes, i18n.language)}</span>
                    </label>
                  </li>
                ))}
              </ul>
            )}
            {selected.length > 0 && space !== null ? (
              <p className={styles.space}>{t("rebuild.space", { size: formatBytes(space, i18n.language) })}</p>
            ) : null}
            <Notice tone="info">{t("rebuild.reinstallHint")}</Notice>
            {error ? <Notice tone="error">{t(`errors.${error}`)}</Notice> : null}
            <div className={styles.footer}>
              <Button variant="ghost" onClick={() => void navigate("/library")}>
                {t("common.cancel")}
              </Button>
              <Button variant="primary" disabled={selected.length === 0} onClick={start}>
                {t("rebuild.start", { count: selected.length })}
              </Button>
            </div>
          </Panel>
        )}
      </main>
    </div>
  );
}

function QueueRow({ item, icon }: { item: RebuildItem; icon: string | null }) {
  const { t } = useTranslation();
  const state = item.state;
  return (
    <li className={styles.row}>
      <span className={styles.art}>
        <GameArt title={item.title} icon={icon} />
      </span>
      <span className={styles.name}>
        <b>{item.title}</b>
        {state.state === "building" ? (
          <>
            <ProgressBar label={t(`stages.${state.stage}`)} value={state.percent} />
            <span>{t(`stages.${state.stage}`)}</span>
          </>
        ) : null}
        {state.state === "failed" ? <span className={styles.error}>{t(`errors.${state.error.code}`)}</span> : null}
      </span>
      <span className={`${styles.chip} ${styles[state.state]}`}>{t(`rebuild.state.${state.state}`)}</span>
    </li>
  );
}
