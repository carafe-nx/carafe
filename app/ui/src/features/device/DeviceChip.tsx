import { useTranslation } from "react-i18next";
import type { DeviceStatus } from "@/shared/api/bindings/DeviceStatus";
import styles from "./DeviceChip.module.css";

export function DeviceChip({ status }: { status: DeviceStatus }) {
  const { t } = useTranslation();
  const label = status.connected
    ? t("device.connected", { via: status.via ?? "MTP" })
    : t("device.disconnected");
  const hint = status.supported ? t("device.hint") : t("device.unsupported");
  return (
    <span className={styles.chip} title={status.connected ? undefined : hint}>
      <span className={status.connected ? styles.on : styles.off} aria-hidden="true" />
      {label}
    </span>
  );
}
