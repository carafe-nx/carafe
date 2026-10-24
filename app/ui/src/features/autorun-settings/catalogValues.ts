import type { TFunction } from "i18next";
import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { Catalog } from "@/shared/api/bindings/Catalog";
import type { Condition } from "@/shared/api/bindings/Condition";
import type { Label } from "@/shared/api/bindings/Label";
import type { Preset } from "@/shared/api/bindings/Preset";
import type { Setting } from "@/shared/api/bindings/Setting";

export function labelText(t: TFunction, label: Label): string {
  switch (label.kind) {
    case "key":
      return t(label.key);
    case "text":
      return label.text;
    case "unit":
      return t(label.key, { value: label.value });
  }
}

export function findSetting(catalog: Catalog, id: string): Setting | undefined {
  for (const section of catalog.sections) {
    for (const entry of section.entries) {
      if (entry.kind === "setting" && entry.id === id) {
        return entry;
      }
      if (entry.kind === "group") {
        for (const page of entry.pages) {
          const found = page.settings.find((setting) => setting.id === id);
          if (found) {
            return found;
          }
        }
      }
    }
  }
  return undefined;
}

function typedIndex(settings: AutorunSettings, setting: Setting): number {
  switch (setting.id) {
    case "cpu":
      return settings.system.cpu === "box64" ? 1 : 0;
    case "d3d":
      return settings.graphics.direct3d === "dxvkVkd3d" ? 1 : settings.graphics.direct3d === "wine" ? 2 : 0;
    case "upscaling":
      return settings.graphics.upscaling === "fsr" ? 1 : settings.graphics.upscaling === "integer" ? 2 : 0;
    case "lsfg":
      return settings.graphics.lsfg ? 1 : 0;
    default:
      return setting.default;
  }
}

export function valueIndex(settings: AutorunSettings, setting: Setting): number {
  if (setting.storage === "typed") {
    return typedIndex(settings, setting);
  }
  const text = settings.options[setting.id];
  const index = setting.values.findIndex((value) => value.text === text);
  return text === undefined || index < 0 ? setting.default : index;
}

function holds(catalog: Catalog, settings: AutorunSettings, condition: Condition): boolean {
  if (condition.kind === "always") {
    return true;
  }
  const target = findSetting(catalog, condition.setting);
  return target ? condition.values.includes(valueIndex(settings, target)) : true;
}

export function isShown(catalog: Catalog, settings: AutorunSettings, condition: Condition): boolean {
  return condition.kind !== "show" || holds(catalog, settings, condition);
}

export function isActive(catalog: Catalog, settings: AutorunSettings, condition: Condition): boolean {
  return condition.kind !== "on" || holds(catalog, settings, condition);
}

export function withValue(settings: AutorunSettings, setting: Setting, index: number): AutorunSettings {
  const options = { ...settings.options };
  const text = setting.values[index]?.text;
  if (index === setting.default || text === null || text === undefined) {
    delete options[setting.id];
  } else {
    options[setting.id] = text;
  }
  return { ...settings, options };
}

export function currentPreset(catalog: Catalog, settings: AutorunSettings, presets: readonly Preset[]): Preset | null {
  return (
    presets.find((preset) =>
      preset.values.every((value) => {
        const setting = findSetting(catalog, value.setting);
        return setting !== undefined && valueIndex(settings, setting) === value.value;
      }),
    ) ?? null
  );
}

export function withPreset(catalog: Catalog, settings: AutorunSettings, preset: Preset): AutorunSettings {
  return preset.values.reduce((next, value) => {
    const setting = findSetting(catalog, value.setting);
    return setting ? withValue(next, setting, value.value) : next;
  }, settings);
}
