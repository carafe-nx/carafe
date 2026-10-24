import styles from "./Flag.module.css";
import br from "./flags/br.svg";
import ca from "./flags/ca.svg";
import cn from "./flags/cn.svg";
import de from "./flags/de.svg";
import es from "./flags/es.svg";
import fr from "./flags/fr.svg";
import gb from "./flags/gb.svg";
import it from "./flags/it.svg";
import jp from "./flags/jp.svg";
import kr from "./flags/kr.svg";
import mx from "./flags/mx.svg";
import nl from "./flags/nl.svg";
import pt from "./flags/pt.svg";
import ru from "./flags/ru.svg";
import tw from "./flags/tw.svg";

const sources = { br, ca, cn, de, es, fr, gb, it, jp, kr, mx, nl, pt, ru, tw } as const;

export type FlagCode = keyof typeof sources;

type FlagProps = {
  code: FlagCode;
};

export function Flag({ code }: FlagProps) {
  return <img className={styles.flag} src={sources[code]} alt="" />;
}
