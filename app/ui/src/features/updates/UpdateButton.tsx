import type { TFunction } from "i18next";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { UpdateStatus } from "@/shared/api/bindings/UpdateStatus";
import { Icon } from "@/shared/ui/Icon";
import { DownloadRing } from "./DownloadRing";
import styles from "./UpdateButton.module.css";
import { UpdatePanel } from "./UpdatePanel";

type Look = {
  tone: "news" | "quiet" | "accent" | "warn";
  icon: ReactNode;
  label: string;
};

function lookOf(status: UpdateStatus, t: TFunction): Look | null {
  const state = status.state;
  switch (state.state) {
    case "idle":
      return null;
    case "available":
      return {
        tone: "news",
        icon: <span className={styles.dot} aria-hidden="true" />,
        label: t("update.pill.available", { version: state.release.version }),
      };
    case "downloading":
      return {
        tone: "quiet",
        icon: <DownloadRing fraction={state.total ? state.received / state.total : null} />,
        label: t("update.pill.downloading", { version: state.release.version }),
      };
    case "ready":
      if (status.busy) {
        return {
          tone: "quiet",
          icon: <Icon name="clock" size={16} />,
          label: t(status.installWhenIdle ? "update.pill.restartsWhenDone" : "update.pill.waiting"),
        };
      }
      return { tone: "accent", icon: <Icon name="restart" size={16} />, label: t("update.pill.ready") };
    case "failed":
      return {
        tone: "warn",
        icon: <Icon name="alert" size={16} />,
        label: t(`update.pill.failed.${state.step}`),
      };
  }
}

export function UpdateButton({ status }: { status: UpdateStatus }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const outside = (event: PointerEvent) => {
      if (root.current && event.target instanceof Node && !root.current.contains(event.target)) {
        setOpen(false);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  const look = status.enabled ? lookOf(status, t) : null;
  if (look === null) {
    return null;
  }
  return (
    <div ref={root} className={styles.root}>
      <button
        type="button"
        className={`${styles.pill} ${styles[look.tone]}`}
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen((shown) => !shown)}
      >
        {look.icon}
        <span>{look.label}</span>
      </button>
      {open ? <UpdatePanel status={status} onDone={() => setOpen(false)} /> : null}
    </div>
  );
}
