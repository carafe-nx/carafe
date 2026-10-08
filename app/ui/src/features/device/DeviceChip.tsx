import { useTranslation } from "react-i18next";
import type { DeviceStatus } from "@/shared/api/bindings/DeviceStatus";
import styles from "./DeviceChip.module.css";

export function DeviceChip({ status }: { status: DeviceStatus }) {
  const { t } = useTranslation();
  const label = status.connected
    ? t("device.connected", { via: status.via ?? "MTP" })
    : t("device.disconnected");
  return (
    <span className={styles.chip} title={status.connected ? undefined : t("device.hint")}>
      <span className={status.connected ? styles.on : styles.off} aria-hidden="true" />
      {label}
    </span>
  );
}
