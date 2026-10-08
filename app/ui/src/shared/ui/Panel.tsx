import type { HTMLAttributes } from "react";
import styles from "./controls.module.css";

export function Panel({ className, ...rest }: HTMLAttributes<HTMLElement>) {
  return <section className={[styles.panel, className].filter(Boolean).join(" ")} {...rest} />;
}
