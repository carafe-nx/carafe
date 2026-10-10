import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate } from "react-router";
import type { GameSummary } from "@/shared/api/bindings/GameSummary";
import type { RebuildOffer } from "@/shared/api/bindings/RebuildOffer";
import { api } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Icon } from "@/shared/ui/Icon";
import styles from "./RebuildBanner.module.css";
import { useRebuildStatus } from "./useRebuildStatus";

type RebuildBannerProps = {
  games: GameSummary[] | null;
  onFinished: () => void;
};

export function RebuildBanner({ games, onFinished }: RebuildBannerProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const status = useRebuildStatus();
  const [offer, setOffer] = useState<RebuildOffer | null>(null);
  const running = useRef(false);

  useEffect(() => {
    if (games === null) {
      return;
    }
    let active = true;
    void api.rebuildOffer().then((next) => {
      if (active) {
        setOffer(next);
      }
    });
    return () => {
      active = false;
    };
  }, [games]);

  useEffect(() => {
    if (status === null) {
      return;
    }
    if (running.current && !status.running) {
      onFinished();
    }
    running.current = status.running;
  }, [status, onFinished]);

  if (status?.running) {
    const index = status.items.findIndex((item) => item.state.state === "building");
    const current = status.items[index];
    return (
      <div className={styles.banner} role="status">
        <span className={`${styles.icon} ${styles.busy}`}>
          <Icon name="rebuild" size={18} />
        </span>
        <span className={styles.text}>
          <b>
            {current
              ? t("rebuild.running", { current: index + 1, total: status.items.length, title: current.title })
              : t("rebuild.stopping")}
          </b>
        </span>
        <Link className={styles.link} to="/rebuild">
          {t("rebuild.details")}
        </Link>
      </div>
    );
  }

  if (offer === null || offer.dismissed || offer.games.length === 0) {
    return null;
  }
  const dismiss = () => {
    void api.dismissRebuildOffer().then(() => setOffer({ ...offer, dismissed: true }));
  };
  return (
    <div className={styles.banner}>
      <span className={styles.icon}>
        <Icon name="rebuild" size={18} />
      </span>
      <span className={styles.text}>
        <b>{t("rebuild.offerTitle", { count: offer.games.length })}</b>
        <span className={styles.muted}>{t("rebuild.offerText", { version: offer.runtimeVersion })}</span>
      </span>
      <span className={styles.actions}>
        <Button variant="ghost" onClick={dismiss}>
          {t("rebuild.notNow")}
        </Button>
        <Button variant="primary" onClick={() => void navigate("/rebuild")}>
          {t("rebuild.open")}
        </Button>
      </span>
    </div>
  );
}
