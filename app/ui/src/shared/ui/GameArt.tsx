import { artPalette } from "./artPalette";
import styles from "./GameArt.module.css";

type GameArtProps = {
  title: string;
  icon?: string | null;
  size?: "tile" | "card";
};

export function GameArt({ title, icon, size = "tile" }: GameArtProps) {
  const className = `${styles.art} ${styles[size]}`;
  if (icon) {
    return <img className={className} src={icon} alt="" />;
  }
  const [from, to] = artPalette(title);
  const letter = title.trim().charAt(0).toUpperCase() || "?";
  return (
    <div className={className} style={{ background: `linear-gradient(140deg, ${from}, ${to})` }} aria-hidden="true">
      {letter}
    </div>
  );
}
