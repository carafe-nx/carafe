import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Language } from "@/shared/api/bindings/Language";
import en from "./locales/en.json";
import ru from "./locales/ru.json";
import zhHans from "./locales/zh-Hans.json";
import { systemLanguage } from "./language";

export function initI18n() {
  void i18n.use(initReactI18next).init({
    resources: { ru: { translation: ru }, en: { translation: en }, "zh-Hans": { translation: zhHans } },
    lng: systemLanguage(navigator.language),
    fallbackLng: "en",
    interpolation: { escapeValue: false },
  });
}

export function applyLanguage(language: Language) {
  const lng = language === "system" ? systemLanguage(navigator.language) : language;
  document.documentElement.lang = lng;
  if (i18n.language !== lng) {
    void i18n.changeLanguage(lng);
  }
}
