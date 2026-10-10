import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/api/commands";
import { Icon } from "@/shared/ui/Icon";
import styles from "./UpdatedToast.module.css";

const announced = new Set<string>();

export function UpdatedToast({ version }: { version: string | null }) {
  const { t } = useTranslation();
  const [visible, setVisible] = useState<string | null>(null);

  useEffect(() => {
    if (version === null || announced.has(version)) {
      return;
    }
    announced.add(version);
    setVisible(version);
  }, [version]);

  if (visible === null) {
    return null;
  }
  return (
    <div className={styles.toast} role="status">
      <span className={styles.tick}>
        <Icon name="check" size={13} />
      </span>
      <span>{t("update.updatedTo", { version: visible })}</span>
      <button type="button" className={styles.link} onClick={() => void api.openRelease(visible)}>
        {t("update.whatsNew")}
      </button>
      <button type="button" className={styles.close} aria-label={t("update.close")} onClick={() => setVisible(null)}>
        <Icon name="plus" size={14} />
      </button>
    </div>
  );
}
