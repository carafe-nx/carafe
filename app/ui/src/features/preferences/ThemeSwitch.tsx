import { useTranslation } from "react-i18next";
import { resolveTheme } from "@/shared/theme/applyTheme";
import { Icon } from "@/shared/ui/Icon";
import styles from "./ThemeSwitch.module.css";
import { usePreferences } from "./usePreferences";

export function ThemeSwitch() {
  const { t } = useTranslation();
  const { preferences, save } = usePreferences();
  const dark = resolveTheme(preferences.theme) === "dark";

  return (
    <button
      type="button"
      role="switch"
      className={styles.switch}
      aria-checked={dark}
      aria-label={t("settings.darkTheme")}
      title={t("settings.darkTheme")}
      onClick={() => void save({ ...preferences, theme: dark ? "light" : "dark" })}
    >
      <span className={styles.track}>
        <Icon name="sun" size={14} />
        <Icon name="moon" size={14} />
      </span>
      <span className={styles.thumb}>
        <Icon name={dark ? "moon" : "sun"} size={14} />
      </span>
    </button>
  );
}
