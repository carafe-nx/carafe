import styles from "./controls.module.css";

type ProgressBarProps = {
  label: string;
  value: number;
};

export function ProgressBar({ label, value }: ProgressBarProps) {
  const percent = Math.max(0, Math.min(100, value));
  return (
    <div className={styles.progress} role="progressbar" aria-label={label} aria-valuenow={Math.round(percent)} aria-valuemin={0} aria-valuemax={100}>
      <div className={styles.progressFill} style={{ width: `${percent}%` }} />
    </div>
  );
}
