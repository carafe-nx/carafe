import styles from "./UpdateButton.module.css";

const RADIUS = 7;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

export function DownloadRing({ fraction }: { fraction: number | null }) {
  const shown = fraction === null ? 0.25 : Math.max(0.04, Math.min(1, fraction));
  return (
    <svg
      className={fraction === null ? `${styles.ring} ${styles.ringSpinning}` : styles.ring}
      width={18}
      height={18}
      viewBox="0 0 18 18"
      aria-hidden="true"
    >
      <circle className={styles.ringTrack} cx={9} cy={9} r={RADIUS} />
      <circle
        className={styles.ringFill}
        cx={9}
        cy={9}
        r={RADIUS}
        strokeDasharray={CIRCUMFERENCE}
        strokeDashoffset={CIRCUMFERENCE * (1 - shown)}
      />
    </svg>
  );
}
