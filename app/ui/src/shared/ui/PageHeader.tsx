import type { ReactNode } from "react";
import { Link } from "react-router";
import { Icon } from "./Icon";
import styles from "./PageHeader.module.css";

type PageHeaderProps = {
  title: string;
  backTo?: string;
  backLabel?: string;
  aside?: ReactNode;
};

export function PageHeader({ title, backTo, backLabel, aside }: PageHeaderProps) {
  return (
    <header className={styles.header}>
      {backTo ? (
        <Link className={styles.back} to={backTo}>
          <Icon name="back" size={16} />
          {backLabel}
        </Link>
      ) : null}
      <h1 className={styles.title}>{title}</h1>
      <div className={styles.aside}>{aside}</div>
    </header>
  );
}
