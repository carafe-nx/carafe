import { type FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ArtGame } from "@/shared/api/bindings/ArtGame";
import type { ArtImage } from "@/shared/api/bindings/ArtImage";
import { api, isCommandError } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Notice } from "@/shared/ui/Notice";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./ArtPicker.module.css";

type ArtPickerProps = {
  initialTerm: string;
  selectedImageId: number | null;
  hasKey: boolean;
  onSaveKey: (key: string) => Promise<void>;
  onPick: (gameId: number, image: ArtImage) => void;
};

export function ArtPicker({ initialTerm, selectedImageId, hasKey, onSaveKey, onPick }: ArtPickerProps) {
  const { t } = useTranslation();
  const [term, setTerm] = useState(initialTerm);
  const [games, setGames] = useState<ArtGame[] | null>(null);
  const [gameId, setGameId] = useState<number | null>(null);
  const [images, setImages] = useState<ArtImage[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [keyDraft, setKeyDraft] = useState("");
  const [keyRejected, setKeyRejected] = useState(false);

  const run = async <T,>(work: () => Promise<T>): Promise<T | null> => {
    setBusy(true);
    setError(null);
    try {
      return await work();
    } catch (failure) {
      const code = isCommandError(failure) ? failure.code : "unknown";
      if (code === "apiKey") {
        setKeyRejected(true);
      } else {
        setError(code);
      }
      return null;
    } finally {
      setBusy(false);
    }
  };

  const findGames = async (text: string) => {
    setGameId(null);
    setImages(null);
    setGames(await run(() => api.artGames(text)));
  };

  const search = (event: FormEvent) => {
    event.preventDefault();
    void findGames(term);
  };

  const saveKey = async (event: FormEvent) => {
    event.preventDefault();
    const key = keyDraft.trim();
    if (key === "") {
      return;
    }
    setBusy(true);
    try {
      await onSaveKey(key);
    } catch (failure) {
      setError(isCommandError(failure) ? failure.code : "unknown");
      return;
    } finally {
      setBusy(false);
    }
    setKeyRejected(false);
    setKeyDraft("");
    if (term.trim() !== "") {
      await findGames(term);
    }
  };

  if (!hasKey || keyRejected) {
    return (
      <form className={styles.picker} onSubmit={(event) => void saveKey(event)}>
        <Notice tone={keyRejected ? "error" : "info"}>
          <p>{keyRejected ? t("art.keyRejected") : t("art.keyNeeded")}</p>
          <ol className={styles.steps}>
            <li>{t("art.keyStep1")}</li>
            <li>{t("art.keyStep2")}</li>
            <li>{t("art.keyStep3")}</li>
          </ol>
        </Notice>
        <div className={styles.searchRow}>
          <TextInput
            type="password"
            autoComplete="off"
            aria-label={t("settings.steamGridDbKey")}
            placeholder={t("settings.steamGridDbKey")}
            value={keyDraft}
            onChange={(event) => setKeyDraft(event.target.value)}
          />
          <Button type="submit" variant="primary" disabled={busy || keyDraft.trim() === ""}>
            {t("art.saveKey")}
          </Button>
        </div>
        {error ? <Notice tone="error">{t(`errors.${error}`)}</Notice> : null}
      </form>
    );
  }

  const openGame = async (id: number) => {
    setGameId(id);
    setImages(null);
    setImages(await run(() => api.artImages(id)));
  };

  return (
    <div className={styles.picker}>
      <form className={styles.searchRow} onSubmit={search}>
        <TextInput aria-label={t("art.search")} placeholder={t("art.search")} value={term} onChange={(event) => setTerm(event.target.value)} />
        <Button type="submit" disabled={busy || term.trim() === ""}>
          {t("art.find")}
        </Button>
      </form>
      {error ? <Notice tone="error">{t(`errors.${error}`)}</Notice> : null}
      {busy ? <p className={styles.muted}>{t("art.loading")}</p> : null}
      {games && games.length === 0 ? <p className={styles.muted}>{t("art.noGames")}</p> : null}
      {games && games.length > 0 ? (
        <ul className={styles.games}>
          {games.map((game) => (
            <li key={game.id}>
              <button
                type="button"
                className={styles.game}
                data-active={game.id === gameId}
                onClick={() => void openGame(game.id)}
              >
                {game.name}
                {game.year ? <span className={styles.muted}> · {game.year}</span> : null}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      {images && images.length === 0 ? <p className={styles.muted}>{t("art.noImages")}</p> : null}
      {images && images.length > 0 && gameId !== null ? (
        <div className={styles.grid} role="listbox" aria-label={t("art.pick")}>
          {images.map((image) => (
            <button
              key={image.id}
              type="button"
              role="option"
              aria-selected={image.id === selectedImageId}
              className={styles.thumb}
              onClick={() => onPick(gameId, image)}
            >
              <img src={image.thumb} alt="" loading="lazy" />
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
