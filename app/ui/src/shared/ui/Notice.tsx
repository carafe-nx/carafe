import type { ReactNode } from "react";
import styles from "./controls.module.css";
import { Icon } from "./Icon";

type NoticeProps = {
  tone: "info" | "warning" | "error";
  children: ReactNode;
};

export function Notice({ tone, children }: NoticeProps) {
  return (
    <div className={`${styles.notice} ${styles[tone]}`} role={tone === "error" ? "alert" : "status"}>
      <Icon name={tone === "info" ? "info" : "alert"} size={16} />
      <div>{children}</div>
    </div>
  );
}
