import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Language } from "@/shared/api/bindings/Language";
import en from "./locales/en.json";
import ru from "./locales/ru.json";

function systemLanguage(): "ru" | "en" {
  return navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
}

export function initI18n() {
  void i18n.use(initReactI18next).init({
    resources: { ru: { translation: ru }, en: { translation: en } },
    lng: systemLanguage(),
    fallbackLng: "en",
    interpolation: { escapeValue: false },
  });
}

export function applyLanguage(language: Language) {
  const lng = language === "system" ? systemLanguage() : language;
  document.documentElement.lang = lng;
  if (i18n.language !== lng) {
    void i18n.changeLanguage(lng);
  }
}
