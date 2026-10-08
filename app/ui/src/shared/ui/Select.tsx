import styles from "./controls.module.css";

type SelectProps<T extends string | number> = {
  id?: string;
  value: T;
  options: readonly { value: T; label: string }[];
  onChange: (value: T) => void;
};

export function Select<T extends string | number>({ id, value, options, onChange }: SelectProps<T>) {
  return (
    <select
      id={id}
      className={styles.select}
      value={String(value)}
      onChange={(event) => {
        const picked = options.find((option) => String(option.value) === event.target.value);
        if (picked) {
          onChange(picked.value);
        }
      }}
    >
      {options.map((option) => (
        <option key={String(option.value)} value={String(option.value)}>
          {option.label}
        </option>
      ))}
    </select>
  );
}
