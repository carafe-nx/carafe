import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { RegImport } from "@/shared/api/bindings/RegImport";
import type { RegistryEntry } from "@/shared/api/bindings/RegistryEntry";
import { Button } from "@/shared/ui/Button";
import styles from "./RegistryEditor.module.css";
import { changeText, entryType, rootShort, sameValue } from "./registryValues";

type RegistryImportDialogProps = {
  fileName: string;
  found: RegImport;
  current: RegistryEntry[];
  onImport: (entries: RegistryEntry[]) => void;
  onCancel: () => void;
};

export function RegistryImportDialog({ fileName, found, current, onImport, onCancel }: RegistryImportDialogProps) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDialogElement>(null);
  const [off, setOff] = useState<ReadonlySet<number>>(new Set());

  useEffect(() => {
    const element = dialog.current;
    if (element && !element.open) {
      element.showModal();
    }
  }, []);

  const chosen = found.entries.filter((_, index) => !off.has(index));
  const toggle = (index: number) => {
    const next = new Set(off);
    if (next.has(index)) {
      next.delete(index);
    } else {
      next.add(index);
    }
    setOff(next);
  };

  return (
    <dialog
      ref={dialog}
      className={styles.dialog}
      aria-labelledby="registry-import-title"
      onCancel={(event) => {
        event.preventDefault();
        onCancel();
      }}
    >
      <h2 id="registry-import-title">{t("registry.importTitle", { name: fileName })}</h2>
      <p>{t("registry.importText")}</p>
      <div className={styles.importList}>
        {found.entries.map((entry, index) => {
          const replaces = current.some((existing) => sameValue(existing, entry));
          const type = entryType(entry.change);
          return (
            <label key={`${entry.root}-${entry.path}-${entry.name}-${index}`} className={styles.importRow}>
              <input type="checkbox" checked={!off.has(index)} onChange={() => toggle(index)} />
              <span className={styles.mono}>
                {rootShort[entry.root]}\{entry.path}
                {entry.change.kind === "deleteKey" ? "" : ` › ${entry.name || "@"}`}
                {replaces ? <span className={styles.replaces}>{t("registry.replaces")}</span> : null}
              </span>
              <span
                className={[styles.type, type.startsWith("delete") ? styles.typeDelete : ""].filter(Boolean).join(" ")}
              >
                {t(`registry.types.${type}`)}
              </span>
              <span className={styles.mono}>{changeText(entry.change)}</span>
            </label>
          );
        })}
      </div>
      {found.skipped.length > 0 ? (
        <details className={styles.skipped}>
          <summary>{t("registry.skipped", { count: found.skipped.length })}</summary>
          <ul>
            {found.skipped.map((skip) => (
              <li key={skip.line}>
                {t("registry.skipLine", { line: skip.line, reason: t(`registry.skipReasons.${skip.reason}`) })}{" "}
                <span className={styles.mono}>{skip.text}</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
      <div className={styles.dialogActions}>
        <Button onClick={onCancel}>{t("common.cancel")}</Button>
        <Button variant="primary" disabled={chosen.length === 0} onClick={() => onImport(chosen)}>
          {t("registry.importCount", { count: chosen.length })}
        </Button>
      </div>
    </dialog>
  );
}
