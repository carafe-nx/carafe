import { useTranslation } from "react-i18next";
import type { RegistryField } from "@/shared/api/bindings/RegistryField";
import type { RegistryKind } from "@/shared/api/bindings/RegistryKind";
import type { RegistryMenuRow } from "@/shared/api/bindings/RegistryMenuRow";
import type { RegistryRoot } from "@/shared/api/bindings/RegistryRoot";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/Field";
import { Select } from "@/shared/ui/Select";
import { TextInput } from "@/shared/ui/TextInput";
import styles from "./RegistryEditor.module.css";
import { kindValid, menuKinds, rootShort } from "./registryValues";

type RegistryMenuEditorProps = {
  rows: RegistryMenuRow[];
  selected: number;
  onSelect: (index: number) => void;
  onChange: (rows: RegistryMenuRow[]) => void;
};

const emptyField: RegistryField = { root: "currentUser", path: "", name: "", kind: "dword" };

function emptyRow(name: string): RegistryMenuRow {
  return { name, hint: "", fields: [{ ...emptyField }], options: [{ label: "", values: [""] }] };
}

export function RegistryMenuEditor({ rows, selected, onSelect, onChange }: RegistryMenuEditorProps) {
  const { t } = useTranslation();
  const row = rows[selected];
  const put = (next: RegistryMenuRow) => onChange(rows.map((item, index) => (index === selected ? next : item)));
  const add = () => {
    onChange([...rows, emptyRow(t("registry.newRow"))]);
    onSelect(rows.length);
  };
  const columns = row ? `minmax(120px, 1.2fr) ${row.fields.map(() => "minmax(90px, 1fr)").join(" ")} 96px` : "";

  return (
    <div className={styles.menu}>
      <div className={styles.menuList}>
        {rows.map((item, index) => (
          <button
            key={`${item.name}-${index}`}
            type="button"
            className={styles.menuItem}
            aria-current={index === selected}
            onClick={() => onSelect(index)}
          >
            <strong>{item.name || t("registry.unnamed")}</strong>
            <small>
              {t("registry.rowSummary", {
                count: item.options.length,
                names: item.fields.map((field) => field.name || "@").join(", "),
              })}
            </small>
          </button>
        ))}
        <Button variant="ghost" onClick={add}>
          {t("registry.addRow")}
        </Button>
      </div>
      {row ? (
        <div className={styles.menuCard}>
          <div className={styles.menuHead}>
            <Field label={t("registry.rowName")} htmlFor="registry-row-name">
              <TextInput
                id="registry-row-name"
                value={row.name}
                onChange={(event) => put({ ...row, name: event.target.value })}
              />
            </Field>
            <div className={styles.preview}>
              <p className={styles.previewCaption}>{t("registry.preview")}</p>
              <div className={styles.previewRow}>
                <span>{row.name || t("registry.unnamed")}</span>
                <strong>{row.options[0]?.label ?? ""}</strong>
              </div>
            </div>
          </div>
          <Field label={t("registry.rowHint")} htmlFor="registry-row-hint">
            <TextInput
              id="registry-row-hint"
              value={row.hint}
              onChange={(event) => put({ ...row, hint: event.target.value })}
            />
          </Field>
          <div>
            <div className={styles.gridHead}>{t("registry.fields")}</div>
            <div className={styles.grid} style={{ gridTemplateColumns: "96px minmax(0, 2.4fr) minmax(0, 1.2fr) 124px 40px" }}>
              {row.fields.map((field, index) => (
                <FieldRow
                  key={index}
                  field={field}
                  removable={row.fields.length > 1}
                  onChange={(next) =>
                    put({ ...row, fields: row.fields.map((item, at) => (at === index ? next : item)) })
                  }
                  onRemove={() =>
                    put({
                      ...row,
                      fields: row.fields.filter((_, at) => at !== index),
                      options: row.options.map((option) => ({
                        ...option,
                        values: option.values.filter((_, at) => at !== index),
                      })),
                    })
                  }
                />
              ))}
            </div>
            <div className={styles.inline}>
              <Button
                variant="ghost"
                disabled={row.fields.length >= 8}
                onClick={() =>
                  put({
                    ...row,
                    fields: [...row.fields, { ...(row.fields[row.fields.length - 1] ?? emptyField), name: "" }],
                    options: row.options.map((option) => ({ ...option, values: [...option.values, ""] })),
                  })
                }
              >
                {t("registry.addField")}
              </Button>
            </div>
          </div>
          <div>
            <div className={styles.grid} style={{ gridTemplateColumns: columns }}>
              <span className={styles.gridHead}>{t("registry.option")}</span>
              {row.fields.map((field, index) => (
                <span key={index} className={styles.gridHead}>
                  {field.name || "@"}
                </span>
              ))}
              <span />
              {row.options.map((_, index) => (
                <OptionRow
                  key={index}
                  row={row}
                  index={index}
                  onChange={(next) =>
                    put({ ...row, options: row.options.map((item, at) => (at === index ? next : item)) })
                  }
                  onRemove={() => put({ ...row, options: row.options.filter((_, at) => at !== index) })}
                />
              ))}
            </div>
            <div className={styles.inline}>
              <Button
                variant="ghost"
                disabled={row.options.length >= 32}
                onClick={() =>
                  put({ ...row, options: [...row.options, { label: "", values: row.fields.map(() => "") }] })
                }
              >
                {t("registry.addOption")}
              </Button>
              <Button
                variant="danger"
                onClick={() => {
                  onChange(rows.filter((_, index) => index !== selected));
                  onSelect(Math.max(0, selected - 1));
                }}
              >
                {t("registry.removeRow")}
              </Button>
            </div>
          </div>
        </div>
      ) : (
        <p className={styles.lead}>{t("registry.noRows")}</p>
      )}
    </div>
  );
}

type FieldRowProps = {
  field: RegistryField;
  removable: boolean;
  onChange: (next: RegistryField) => void;
  onRemove: () => void;
};

function FieldRow({ field, removable, onChange, onRemove }: FieldRowProps) {
  const { t } = useTranslation();
  return (
    <>
      <Select<RegistryRoot>
        value={field.root}
        options={[
          { value: "currentUser", label: rootShort.currentUser },
          { value: "localMachine", label: rootShort.localMachine },
        ]}
        onChange={(root) => onChange({ ...field, root })}
      />
      <TextInput
        aria-label={t("registry.path")}
        placeholder={t("registry.path")}
        value={field.path}
        spellCheck={false}
        onChange={(event) => onChange({ ...field, path: event.target.value })}
      />
      <TextInput
        aria-label={t("registry.name")}
        placeholder={t("registry.name")}
        value={field.name}
        spellCheck={false}
        onChange={(event) => onChange({ ...field, name: event.target.value })}
      />
      <Select<RegistryKind>
        value={field.kind}
        options={menuKinds.map((kind) => ({ value: kind, label: t(`registry.types.${kind}`) }))}
        onChange={(kind) => onChange({ ...field, kind })}
      />
      <Button variant="ghost" disabled={!removable} onClick={onRemove} aria-label={t("registry.removeField")}>
        ×
      </Button>
    </>
  );
}

type OptionRowProps = {
  row: RegistryMenuRow;
  index: number;
  onChange: (next: RegistryMenuRow["options"][number]) => void;
  onRemove: () => void;
};

function OptionRow({ row, index, onChange, onRemove }: OptionRowProps) {
  const { t } = useTranslation();
  const option = row.options[index];
  if (!option) {
    return null;
  }
  return (
    <>
      <TextInput
        aria-label={t("registry.option")}
        placeholder={t("registry.optionLabel")}
        value={option.label}
        onChange={(event) => onChange({ ...option, label: event.target.value })}
      />
      {row.fields.map((field, at) => {
        const text = option.values[at] ?? "";
        return (
          <TextInput
            key={at}
            aria-label={field.name || "@"}
            aria-invalid={!kindValid(field.kind, text)}
            value={text}
            spellCheck={false}
            onChange={(event) =>
              onChange({ ...option, values: option.values.map((value, i) => (i === at ? event.target.value : value)) })
            }
          />
        );
      })}
      <Button
        variant="ghost"
        disabled={row.options.length < 2}
        onClick={onRemove}
        aria-label={t("registry.removeOption")}
      >
        ×
      </Button>
    </>
  );
}
