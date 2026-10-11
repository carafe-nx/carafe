import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { BuildBanner } from "@/features/build/BuildBanner";
import { busyReason } from "@/features/build/busyReason";
import { useBuildStatus } from "@/features/build/useBuildStatus";
import { DeviceChip } from "@/features/device/DeviceChip";
import { useDeviceStatus } from "@/features/device/useDeviceStatus";
import { RebuildBanner } from "@/features/rebuild/RebuildBanner";
import { useRebuildStatus } from "@/features/rebuild/useRebuildStatus";
import { UpdateButton } from "@/features/updates/UpdateButton";
import { UpdatedToast } from "@/features/updates/UpdatedToast";
import { useUpdateStatus } from "@/features/updates/useUpdateStatus";
import type { TitleId } from "@/shared/api/bindings/TitleId";
import { formatBytes } from "@/shared/format";
import { CarafeMark } from "@/shared/ui/CarafeMark";
import { CarafeWord } from "@/shared/ui/CarafeWord";
import { Icon } from "@/shared/ui/Icon";
import { GameCard } from "./GameCard";
import { GameTile } from "./GameTile";
import styles from "./LibraryPage.module.css";
import { useLibrary } from "./useLibrary";

export function LibraryPage() {
  const { t, i18n } = useTranslation();
  const { games, reload } = useLibrary();
  const device = useDeviceStatus();
  const update = useUpdateStatus();
  const [selectedId, setSelectedId] = useState<TitleId | null>(null);
  const selected = games?.find((game) => game.titleId === selectedId) ?? null;
  const totalBytes = games?.reduce((sum, game) => sum + game.sizeBytes, 0) ?? 0;
  const build = useBuildStatus();
  const rebuildStatus = useRebuildStatus();
  const busy = busyReason(t, build, rebuildStatus?.running ?? false);
  const onBuildFinished = useCallback(() => void reload(), [reload]);

  return (
    <div className={styles.page}>
      <header className={styles.topBar}>
        <span className={styles.brand}>
          <CarafeMark size={32} />
          <CarafeWord cell={3} />
        </span>
        <span className={styles.spacer} />
        {update ? <UpdateButton status={update} /> : null}
        <DeviceChip status={device} />
        <Link className={styles.iconLink} to="/settings" aria-label={t("settings.title")} title={t("settings.title")}>
          <Icon name="gear" />
        </Link>
      </header>

      <main className={styles.body}>
        <section className={styles.shelf} aria-labelledby="library-title">
          <div className={styles.shelfHead}>
            <h1 id="library-title">{t("library.title")}</h1>
            {games && games.length > 0 ? (
              <span className={styles.muted}>
                {t("library.count", { count: games.length })} · {formatBytes(totalBytes, i18n.language)}
              </span>
            ) : null}
          </div>
          <BuildBanner status={build} knownGames={games?.map((game) => game.titleId) ?? []} onFinished={onBuildFinished} />
          <RebuildBanner games={games} onFinished={() => void reload()} />
          <div className={styles.tiles}>
            {games?.map((game) => (
              <GameTile
                key={game.titleId}
                game={game}
                selected={game.titleId === selectedId}
                onSelect={() => setSelectedId(game.titleId === selectedId ? null : game.titleId)}
              />
            ))}
            {busy ? (
              <span className={`${styles.tile} ${styles.tileDisabled}`} aria-disabled="true" title={busy}>
                <span className={styles.addArt}>
                  <Icon name="plus" size={30} />
                </span>
                <span className={styles.tileName}>{t("library.newGame")}</span>
              </span>
            ) : (
              <Link className={styles.tile} to="/new">
                <span className={styles.addArt}>
                  <Icon name="plus" size={30} />
                </span>
                <span className={styles.tileName}>{t("library.newGame")}</span>
              </Link>
            )}
          </div>
          {busy ? <p className={styles.muted}>{busy}</p> : null}
          {games && games.length === 0 ? <p className={styles.empty}>{t("library.empty")}</p> : null}
        </section>

        <aside className={styles.side}>
          {selected ? (
            <GameCard
              key={selected.titleId}
              game={selected}
              connected={device.connected}
              busyReason={busy}
              onDeleted={() => {
                setSelectedId(null);
                void reload();
              }}
            />
          ) : (
            <div className={styles.hint}>
              <Icon name="switch" size={28} />
              <p>{t("library.selectHint")}</p>
            </div>
          )}
        </aside>
      </main>
      <UpdatedToast version={update?.updatedTo ?? null} />
    </div>
  );
}
