import word from "@assets/logo/word.png";
import styles from "./CarafeLogo.module.css";

type CarafeWordProps = {
  cell: 2 | 3 | 4;
};

export function CarafeWord({ cell }: CarafeWordProps) {
  return (
    <span
      className={styles.word}
      style={{ width: 35 * cell, height: 7 * cell, maskImage: `url(${word})` }}
      role="img"
      aria-label="Carafe"
    />
  );
}
