import styles from "./Flag.module.css";
import gb from "./flags/gb.svg";
import ru from "./flags/ru.svg";

const sources = { gb, ru } as const;

export type FlagCode = keyof typeof sources;

type FlagProps = {
  code: FlagCode;
};

export function Flag({ code }: FlagProps) {
  return <img className={styles.flag} src={sources[code]} alt="" />;
}
