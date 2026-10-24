import { useTranslation } from "react-i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { Setting } from "@/shared/api/bindings/Setting";
import { Field } from "@/shared/ui/Field";
import { Select } from "@/shared/ui/Select";
import { isActive, isShown, labelText, valueIndex, withValue } from "./catalogValues";

type CatalogFieldProps = {
  catalog: Catalog;
  setting: Setting;
  value: AutorunSettings;
  onChange: (next: AutorunSettings) => void;
};

export function CatalogField({ catalog, setting, value, onChange }: CatalogFieldProps) {
  const { t } = useTranslation();
  if (!isShown(catalog, value, setting.condition)) {
    return null;
  }
  const id = `catalog-${setting.id}`;
  return (
    <Field label={t(setting.name)} htmlFor={id} hint={t(setting.hint)}>
      <Select<number>
        id={id}
        disabled={!isActive(catalog, value, setting.condition) || setting.values.length < 2}
        value={valueIndex(value, setting)}
        options={setting.values.map((option, index) => ({ value: index, label: labelText(t, option.label) }))}
        onChange={(index) => onChange(withValue(value, setting, index))}
      />
    </Field>
  );
}
