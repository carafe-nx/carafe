import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { RegistryEntry } from "@/shared/api/bindings/RegistryEntry";
import type { RegistryRoot } from "@/shared/api/bindings/RegistryRoot";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/Field";
import { Select } from "@/shared/ui/Select";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./RegistryEditor.module.css";
import { type EntryType, changeText, editableTypes, entryType, parseChange } from "./registryValues";

type RegistryEntryEditorProps = {
  id: string;
  entry: RegistryEntry;
  canMakeRow: boolean;
  onChange: (next: RegistryEntry) => void;
  onRemove: () => void;
  onMakeRow: () => void;
};

export function RegistryEntryEditor({ id, entry, canMakeRow, onChange, onRemove, onMakeRow }: RegistryEntryEditorProps) {
  const { t } = useTranslation();
  const type = entryType(entry.change);
  const otherKind = entry.change.kind === "set" && entry.change.value.type === "other" ? entry.change.value.kind : 0;
  const [text, setText] = useState(changeText(entry.change));
  const [invalid, setInvalid] = useState(false);
  const types = editableTypes.includes(type) ? editableTypes : [...editableTypes, type];
  const deleting = type === "deleteValue" || type === "deleteKey";

  const apply = (nextType: EntryType, nextText: string) => {
    const change = parseChange(nextType, nextText, otherKind);
    setInvalid(change === null);
    if (change) {
      onChange({ ...entry, name: nextType === "deleteKey" ? "" : entry.name, change });
    }
  };

  return (
    <div className={styles.edit}>
      <Field label={t("registry.root")} htmlFor={`${id}-root`}>
        <Select<RegistryRoot>
          id={`${id}-root`}
          value={entry.root}
          options={[
            { value: "currentUser", label: "HKEY_CURRENT_USER" },
            { value: "localMachine", label: "HKEY_LOCAL_MACHINE" },
          ]}
          onChange={(root) => onChange({ ...entry, root })}
        />
      </Field>
      <Field label={t("registry.path")} htmlFor={`${id}-path`}>
        <TextInput
          id={`${id}-path`}
          value={entry.path}
          spellCheck={false}
          onChange={(event) => onChange({ ...entry, path: event.target.value })}
        />
      </Field>
      <Field label={t("registry.name")} htmlFor={`${id}-name`} hint={t("registry.nameHint")}>
        <TextInput
          id={`${id}-name`}
          value={entry.name}
          spellCheck={false}
          disabled={type === "deleteKey"}
          onChange={(event) => onChange({ ...entry, name: event.target.value })}
        />
      </Field>
      <Field label={t("registry.type")} htmlFor={`${id}-type`}>
        <Select<EntryType>
          id={`${id}-type`}
          value={type}
          options={types.map((value) => ({ value, label: t(`registry.types.${value}`) }))}
          onChange={(nextType) => {
            const nextText = nextType === type ? text : "";
            setText(nextText);
            apply(nextType, nextType === "dword" || nextType === "qword" ? nextText || "0" : nextText);
          }}
        />
      </Field>
      <div className={styles.editWide}>
        <Field
          label={t("registry.value")}
          htmlFor={`${id}-value`}
          hint={invalid ? <span className={styles.error}>{t(`registry.invalid.${type}`)}</span> : t(`registry.valueHints.${type}`)}
        >
          <TextInput
            id={`${id}-value`}
            value={deleting ? "" : text}
            spellCheck={false}
            disabled={deleting}
            onChange={(event) => {
              setText(event.target.value);
              apply(type, event.target.value);
            }}
          />
        </Field>
      </div>
      <div className={styles.editActions}>
        {canMakeRow ? (
          <Button variant="ghost" onClick={onMakeRow}>
            {t("registry.makeRow")}
          </Button>
        ) : null}
        <Button variant="danger" onClick={onRemove}>
          {t("registry.remove")}
        </Button>
      </div>
    </div>
  );
}
