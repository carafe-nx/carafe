import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { DeviceChip } from "@/features/device/DeviceChip";
import { useDeviceStatus } from "@/features/device/useDeviceStatus";
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
  const [selectedId, setSelectedId] = useState<TitleId | null>(null);
  const selected = games?.find((game) => game.titleId === selectedId) ?? null;
  const totalBytes = games?.reduce((sum, game) => sum + game.sizeBytes, 0) ?? 0;

  return (
    <div className={styles.page}>
      <header className={styles.topBar}>
        <span className={styles.brand}>
          <CarafeMark size={32} />
          <CarafeWord cell={3} />
        </span>
        <span className={styles.spacer} />
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
          <div className={styles.tiles}>
            {games?.map((game) => (
              <GameTile
                key={game.titleId}
                game={game}
                selected={game.titleId === selectedId}
                onSelect={() => setSelectedId(game.titleId === selectedId ? null : game.titleId)}
              />
            ))}
            <Link className={styles.tile} to="/new">
              <span className={styles.addArt}>
                <Icon name="plus" size={30} />
              </span>
              <span className={styles.tileName}>{t("library.newGame")}</span>
            </Link>
          </div>
          {games && games.length === 0 ? <p className={styles.empty}>{t("library.empty")}</p> : null}
        </section>

        <aside className={styles.side}>
          {selected ? (
            <GameCard
              key={selected.titleId}
              game={selected}
              connected={device.connected}
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
    </div>
  );
}
