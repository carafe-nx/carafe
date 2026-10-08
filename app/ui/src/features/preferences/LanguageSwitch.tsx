import { useTranslation } from "react-i18next";
import type { Language } from "@/shared/api/bindings/Language";
import { Flag, type FlagCode } from "@/shared/ui/Flag";
import styles from "./LanguageSwitch.module.css";
import { usePreferences } from "./usePreferences";

type LanguageOption = {
  value: Exclude<Language, "system">;
  flag: FlagCode;
  name: string;
};

const RUSSIAN: LanguageOption = { value: "ru", flag: "ru", name: "Русский" };
const ENGLISH: LanguageOption = { value: "en", flag: "gb", name: "English" };
const LANGUAGES = [RUSSIAN, ENGLISH] as const;

export function LanguageSwitch() {
  const { t, i18n } = useTranslation();
  const { preferences, save } = usePreferences();
  const current = LANGUAGES.find((option) => option.value === i18n.language) ?? ENGLISH;
  const others = LANGUAGES.filter((option) => option !== current);

  return (
    <div className={styles.switch}>
      <button
        type="button"
        className={styles.button}
        aria-label={`${t("settings.language")}: ${current.name}`}
        aria-haspopup="true"
        title={current.name}
      >
        <Flag code={current.flag} />
      </button>
      <div className={styles.menu}>
        <ul className={styles.list}>
          {others.map((option) => (
            <li key={option.value}>
              <button
                type="button"
                className={styles.button}
                aria-label={option.name}
                title={option.name}
                onClick={() => void save({ ...preferences, language: option.value })}
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
