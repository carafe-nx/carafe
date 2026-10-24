import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Language } from "@/shared/api/bindings/Language";
import de from "./locales/de.json";
import en from "./locales/en.json";
import es from "./locales/es.json";
import es419 from "./locales/es-419.json";
import fr from "./locales/fr.json";
import frCa from "./locales/fr-CA.json";
import it from "./locales/it.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import nl from "./locales/nl.json";
import pt from "./locales/pt.json";
import ptBr from "./locales/pt-BR.json";
import ru from "./locales/ru.json";
import zhHans from "./locales/zh-Hans.json";
import zhHant from "./locales/zh-Hant.json";
import { type InterfaceLanguage, systemLanguage } from "./language";

const TRANSLATIONS: Record<string, object> = {
  en,
  ru,
  "zh-Hans": zhHans,
  ja,
  ko,
  "zh-Hant": zhHant,
  fr,
  "fr-CA": frCa,
  de,
  it,
  es,
  "es-419": es419,
  nl,
  "pt-PT": pt,
  "pt-BR": ptBr,
};

function localeOf(language: InterfaceLanguage): string {
  return language === "pt" ? "pt-PT" : language;
}

export function initI18n() {
  const resources = Object.fromEntries(
    Object.entries(TRANSLATIONS).map(([locale, translation]) => [locale, { translation }]),
  );
  void i18n.use(initReactI18next).init({
    resources,
    lng: localeOf(systemLanguage(navigator.language)),
    fallbackLng: "en",
    interpolation: { escapeValue: false },
  });
}

export function applyLanguage(language: Language) {
  const lng = localeOf(language === "system" ? systemLanguage(navigator.language) : language);
  document.documentElement.lang = lng;
  if (i18n.language !== lng) {
    void i18n.changeLanguage(lng);
  }
}
