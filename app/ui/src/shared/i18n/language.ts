import type { Language } from "../api/bindings/Language";

export type InterfaceLanguage = Exclude<Language, "system">;

const SAME_NAME: readonly InterfaceLanguage[] = ["ru", "ja", "ko", "de", "it", "nl", "en"];

export function systemLanguage(locale: string): InterfaceLanguage {
  const [language = "", ...rest] = locale.toLowerCase().replaceAll("_", "-").split("-");
  const region = rest.find((part) => part.length === 2 || /^\d{3}$/.test(part));
  const script = rest.find((part) => part.length === 4);
  if (language === "zh") {
    return script === "hant" || (script !== "hans" && ["tw", "hk", "mo"].includes(region ?? ""))
      ? "zh-Hant"
      : "zh-Hans";
  }
  if (language === "fr") return region === "ca" ? "fr-CA" : "fr";
  if (language === "es") return region === undefined || region === "es" ? "es" : "es-419";
  if (language === "pt") return region === undefined || region === "br" ? "pt-BR" : "pt";
  return SAME_NAME.find((code) => code === language) ?? "en";
}
