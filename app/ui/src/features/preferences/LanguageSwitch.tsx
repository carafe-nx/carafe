import { useTranslation } from "react-i18next";
import type { Language } from "@/shared/api/bindings/Language";
import { type InterfaceLanguage, systemLanguage } from "@/shared/i18n/language";
import { Flag, type FlagCode } from "@/shared/ui/Flag";
import styles from "./LanguageSwitch.module.css";
import { usePreferences } from "./usePreferences";

type LanguageOption = {
  value: InterfaceLanguage;
  flag: FlagCode;
  name: string;
};

const LANGUAGES: readonly LanguageOption[] = [
  { value: "de", flag: "de", name: "Deutsch" },
  { value: "en", flag: "gb", name: "English" },
  { value: "es", flag: "es", name: "Español" },
  { value: "es-419", flag: "mx", name: "Español (Latinoamérica)" },
  { value: "fr", flag: "fr", name: "Français" },
  { value: "fr-CA", flag: "ca", name: "Français (Canada)" },
  { value: "it", flag: "it", name: "Italiano" },
  { value: "nl", flag: "nl", name: "Nederlands" },
  { value: "pt", flag: "pt", name: "Português" },
  { value: "pt-BR", flag: "br", name: "Português (Brasil)" },
  { value: "ru", flag: "ru", name: "Русский" },
  { value: "ja", flag: "jp", name: "日本語" },
  { value: "ko", flag: "kr", name: "한국어" },
  { value: "zh-Hans", flag: "cn", name: "简体中文" },
  { value: "zh-Hant", flag: "tw", name: "繁體中文" },
];

const GLOBE = (
  <svg className={styles.globe} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
  </svg>
);

function optionOf(language: InterfaceLanguage): LanguageOption {
  return LANGUAGES.find((option) => option.value === language) ?? { value: "en", flag: "gb", name: "English" };
}

type LanguageSwitchProps = {
  align?: "start" | "end";
};

export function LanguageSwitch({ align = "start" }: LanguageSwitchProps) {
  const { t } = useTranslation();
  const { preferences, save } = usePreferences();
  const choose = (language: Language) => void save({ ...preferences, language });
  const fromSystem = optionOf(systemLanguage(navigator.language));
  const systemName = `${t("settings.system")} · ${fromSystem.name}`;
  const current = preferences.language === "system" ? null : optionOf(preferences.language);
  const others = LANGUAGES.filter((option) => option !== current);

  return (
    <div className={align === "end" ? `${styles.switch} ${styles.end}` : styles.switch}>
      <button
        type="button"
        className={styles.button}
        aria-label={`${t("settings.language")}: ${current?.name ?? systemName}`}
        aria-haspopup="true"
        title={current?.name ?? systemName}
      >
        {current ? <Flag code={current.flag} /> : GLOBE}
      </button>
      <div className={styles.menu}>
        <ul className={styles.list}>
          {current ? (
            <li>
              <button
                type="button"
                className={styles.button}
                aria-label={systemName}
                title={systemName}
                onClick={() => choose("system")}
              >
                {GLOBE}
              </button>
            </li>
          ) : null}
          {others.map((option) => (
            <li key={option.value}>
              <button
                type="button"
                className={styles.button}
                lang={option.value}
                aria-label={option.name}
                title={option.name}
                onClick={() => choose(option.value)}
              >
                <Flag code={option.flag} />
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
