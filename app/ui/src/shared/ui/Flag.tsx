import styles from "./Flag.module.css";
import gb from "./flags/gb.svg";
import ru from "./flags/ru.svg";
import cn from "./flags/cn.svg";

const sources = { gb, ru, cn } as const;

export type FlagCode = keyof typeof sources;

type FlagProps = {
  code: FlagCode;
};

export function Flag({ code }: FlagProps) {
  return <img className={styles.flag} src={sources[code]} alt="" />;
}
