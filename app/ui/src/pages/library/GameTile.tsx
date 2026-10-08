import { useTranslation } from "react-i18next";
import type { GameSummary } from "@/shared/api/bindings/GameSummary";
import { formatBytes } from "@/shared/format";
import { GameArt } from "@/shared/ui/GameArt";
import styles from "./LibraryPage.module.css";

type GameTileProps = {
  game: GameSummary;
  selected: boolean;
  onSelect: () => void;
};

export function GameTile({ game, selected, onSelect }: GameTileProps) {
  const { i18n, t } = useTranslation();
  return (
    <button type="button" className={styles.tile} aria-pressed={selected} onClick={onSelect}>
      <span className={styles.tileArt}>
        <GameArt title={game.title} icon={game.icon} />
        {game.runtimeOutdated ? <span className={styles.outdatedDot} title={t("card.runtimeOutdatedShort")} /> : null}
      </span>
      <span className={styles.tileName}>{game.title}</span>
      <span className={styles.tileMeta}>{formatBytes(game.sizeBytes, i18n.language)}</span>
    </button>
  );
}
