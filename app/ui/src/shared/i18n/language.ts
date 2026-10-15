import type { Language } from "../api/bindings/Language";

export function systemLanguage(locale: string): Exclude<Language, "system"> {
  const language = locale.toLowerCase().replaceAll("_", "-");
  if (language.startsWith("ru")) return "ru";
  if (/^zh(?:-hans(?:-|$)|-(?:cn|sg)$|$)/.test(language)) return "zh-Hans";
  return "en";
}
