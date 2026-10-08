import markDark from "@assets/logo/mark-dark.png";
import markLight from "@assets/logo/mark-light.png";
import styles from "./CarafeLogo.module.css";

type CarafeMarkProps = {
  size: 16 | 32 | 48 | 64;
};

export function CarafeMark({ size }: CarafeMarkProps) {
  return (
    <>
      <img className={`${styles.image} ${styles.onLight}`} src={markLight} width={size} height={size} alt="" />
      <img className={`${styles.image} ${styles.onDark}`} src={markDark} width={size} height={size} alt="" />
    </>
  );
}
