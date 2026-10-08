import styles from "./controls.module.css";

type ToggleProps = {
  id: string;
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

export function Toggle({ id, label, hint, checked, onChange }: ToggleProps) {
  return (
    <label className={styles.toggle} htmlFor={id}>
      <input id={id} type="checkbox" role="switch" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <span className={styles.toggleText}>
        <span>{label}</span>
        {hint ? <span className={styles.fieldHint}>{hint}</span> : null}
      </span>
    </label>
  );
}
