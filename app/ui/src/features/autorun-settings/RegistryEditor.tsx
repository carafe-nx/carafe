import { open, save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { RegImport } from "@/shared/api/bindings/RegImport";
import type { RegistryEntry } from "@/shared/api/bindings/RegistryEntry";
import type { RegistryKind } from "@/shared/api/bindings/RegistryKind";
import type { RegistrySettings } from "@/shared/api/bindings/RegistrySettings";
import type { RegistryWriteMode } from "@/shared/api/bindings/RegistryWriteMode";
import { api, isCommandError } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { Notice } from "@/shared/ui/Notice";
import { Segmented } from "@/shared/ui/Segmented";
import styles from "./RegistryEditor.module.css";
import { RegistryEntryEditor } from "./RegistryEntryEditor";
import { RegistryImportDialog } from "./RegistryImportDialog";
import { RegistryMenuEditor } from "./RegistryMenuEditor";
import { changeText, entryType, mergeEntries, rootShort } from "./registryValues";

type RegistryEditorProps = {
  value: RegistrySettings;
  onChange: (next: RegistrySettings) => void;
};

type Pending = { fileName: string; found: RegImport };

const regFilter = [{ name: "Windows Registry", extensions: ["reg"] }];

function menuKind(entry: RegistryEntry): RegistryKind | null {
  if (entry.change.kind !== "set") {
    return null;
  }
  const type = entry.change.value.type;
  return type === "string" || type === "expandString" || type === "dword" || type === "qword" ? type : null;
}

export function RegistryEditor({ value, onChange }: RegistryEditorProps) {
  const { t } = useTranslation();
  const [expandedIndex, setExpanded] = useState<number | null>(null);
  const [row, setRow] = useState(0);
  const [pending, setPending] = useState<Pending | null>(null);
  const [message, setMessage] = useState<{ tone: "info" | "error"; text: string } | null>(null);

  const setEntries = (entries: RegistryEntry[]) => onChange({ ...value, entries });

  const importFile = async () => {
    const path = await open({ multiple: false, filters: regFilter });
    if (typeof path !== "string") {
      return;
    }
    setMessage(null);
    try {
      const found = await api.importRegistry(path);
      const fileName = path.split(/[\\/]/).pop() ?? path;
      setPending({ fileName, found });
    } catch (error) {
      const code = isCommandError(error) ? error.code : "io";
      setMessage({ tone: "error", text: t(`registry.importErrors.${code === "unsupported" ? "format" : "read"}`) });
    }
  };

  const exportFile = async () => {
    const path = await save({ filters: regFilter, defaultPath: "registry.reg" });
    if (typeof path !== "string") {
      return;
    }
    try {
      await api.exportRegistry(path, value.entries);
      setMessage({ tone: "info", text: t("registry.exported", { count: value.entries.length }) });
    } catch {
      setMessage({ tone: "error", text: t("registry.exportError") });
    }
  };

  const addEntry = () => {
    const last = value.entries[value.entries.length - 1];
    const entry: RegistryEntry = {
      root: last?.root ?? "currentUser",
      path: last?.path ?? "Software\\",
      name: "",
      change: { kind: "set", value: { type: "dword", number: 0 } },
    };
    setEntries([...value.entries, entry]);
    setExpanded(value.entries.length);
  };

  const menuOf = (entry: RegistryEntry) =>
    value.menu.find((item) =>
      item.fields.some(
        (field) =>
          field.root === entry.root &&
          field.path.toLowerCase() === entry.path.toLowerCase() &&
          field.name.toLowerCase() === entry.name.toLowerCase(),
      ),
    );

  const makeRow = (entry: RegistryEntry) => {
    const kind = menuKind(entry);
    if (!kind) {
      return;
    }
    const current = changeText(entry.change);
    const next = {
      name: entry.name || "@",
      hint: "",
      fields: [{ root: entry.root, path: entry.path, name: entry.name, kind }],
      options: [{ label: current, values: [current] }],
    };
    onChange({ ...value, menu: [...value.menu, next] });
    setRow(value.menu.length);
  };

  const empty = value.entries.length === 0;

  return (
    <div className={styles.editor}>
      <p className={styles.lead}>{t("registry.lead")}</p>
      <div className={styles.tools}>
        <Button onClick={() => void importFile()}>{t("registry.import")}</Button>
        <Button onClick={addEntry}>{t("registry.add")}</Button>
        {empty ? null : <Button onClick={() => void exportFile()}>{t("registry.export")}</Button>}
        <div className={styles.mode}>
          <div className={styles.modeRow}>
            {t("registry.mode")}
            <Segmented<RegistryWriteMode>
              label={t("registry.mode")}
              value={value.mode}
              options={[
                { value: "once", label: t("registry.modes.once") },
                { value: "everyStart", label: t("registry.modes.everyStart") },
              ]}
              onChange={(mode) => onChange({ ...value, mode })}
            />
          </div>
          <span className={styles.modeHint}>{t(`registry.modeHints.${value.mode}`)}</span>
        </div>
      </div>
      {message ? <Notice tone={message.tone}>{message.text}</Notice> : null}

      {empty ? (
        <div className={styles.empty}>
          <h3>{t("registry.emptyTitle")}</h3>
          <p>{t("registry.emptyText")}</p>
          <div className={styles.emptyActions}>
            <Button variant="primary" onClick={() => void importFile()}>
              {t("registry.import")}
            </Button>
            <Button onClick={addEntry}>{t("registry.add")}</Button>
          </div>
        </div>
      ) : (
        <div className={styles.table}>
          <div className={`${styles.row} ${styles.head}`}>
            <span>{t("registry.path")}</span>
            <span>{t("registry.name")}</span>
            <span>{t("registry.type")}</span>
            <span>{t("registry.value")}</span>
            <span>{t("registry.inMenu")}</span>
          </div>
          {value.entries.map((entry, index) => {
            const type = entryType(entry.change);
            const menu = menuOf(entry);
            const expanded = expandedIndex === index;
            return (
              <div key={index}>
                <button
                  type="button"
                  className={styles.row}
                  aria-expanded={expanded}
                  onClick={() => setExpanded(expanded ? null : index)}
                >
                  <span className={styles.mono}>
                    {rootShort[entry.root]}\{entry.path}
                  </span>
                  <span className={styles.mono}>
                    {type === "deleteKey" ? "" : entry.name || "@"}
                  </span>
                  <span
                    className={[styles.type, type.startsWith("delete") ? styles.typeDelete : ""]
                      .filter(Boolean)
                      .join(" ")}
                  >
                    {t(`registry.types.${type}`)}
                  </span>
                  <span className={styles.mono}>
                    {changeText(entry.change)}
                  </span>
                  <span className={menu ? styles.menuName : styles.noMenu}>
                    {menu ? menu.name : t("registry.notInMenu")}
                  </span>
                </button>
                {expanded ? (
                  <RegistryEntryEditor
                    key={`edit-${index}`}
                    id={`registry-entry-${index}`}
                    entry={entry}
                    canMakeRow={menuKind(entry) !== null && !menu}
                    onChange={(next) => setEntries(value.entries.map((item, at) => (at === index ? next : item)))}
                    onRemove={() => {
                      setEntries(value.entries.filter((_, at) => at !== index));
                      setExpanded(null);
                    }}
                    onMakeRow={() => makeRow(entry)}
                  />
                ) : null}
              </div>
            );
          })}
        </div>
      )}

      <div className={styles.section}>
        <h3>{t("registry.menuTitle")}</h3>
        <span>{t("registry.menuText")}</span>
      </div>
      <RegistryMenuEditor
        rows={value.menu}
        selected={Math.min(row, Math.max(0, value.menu.length - 1))}
        onSelect={setRow}
        onChange={(menu) => onChange({ ...value, menu })}
      />

      {pending ? (
        <RegistryImportDialog
          fileName={pending.fileName}
          found={pending.found}
          current={value.entries}
          onCancel={() => setPending(null)}
          onImport={(entries) => {
            setEntries(mergeEntries(value.entries, entries));
            setMessage({
              tone: "info",
              text: t("registry.imported", { count: entries.length, name: pending.fileName }),
            });
            setPending(null);
          }}
        />
      ) : null}
    </div>
  );
}
