import styles from "./Spinner.module.css";

type SpinnerProps = {
  size?: "small" | "large";
};

export function Spinner({ size = "small" }: SpinnerProps) {
  return <span className={size === "large" ? `${styles.spinner} ${styles.large}` : styles.spinner} aria-hidden="true" />;
}
