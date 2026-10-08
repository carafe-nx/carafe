import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import { api } from "@/shared/api/commands";
import type { Preferences } from "@/shared/api/bindings/Preferences";
import { applyLanguage } from "@/shared/i18n";
import { applyTheme } from "@/shared/theme/applyTheme";
import { PreferencesContext } from "./PreferencesContext";

export function PreferencesProvider({ children }: { children: ReactNode }) {
  const [preferences, setPreferences] = useState<Preferences | null>(null);

  useEffect(() => {
    void api.getPreferences().then(setPreferences);
  }, []);

  useEffect(() => {
    if (!preferences) {
      return;
    }
    applyLanguage(preferences.language);
    return applyTheme(preferences.theme);
  }, [preferences?.theme, preferences?.language]);

  const save = useCallback(async (next: Preferences) => {
    setPreferences(await api.savePreferences(next));
  }, []);

  const value = useMemo(() => (preferences ? { preferences, save } : null), [preferences, save]);

  if (!value) {
    return null;
  }
  return <PreferencesContext.Provider value={value}>{children}</PreferencesContext.Provider>;
}
