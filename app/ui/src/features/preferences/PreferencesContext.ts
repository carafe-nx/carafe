import { createContext } from "react";
import type { Preferences } from "@/shared/api/bindings/Preferences";

export type PreferencesContextValue = {
  preferences: Preferences;
  save: (next: Preferences) => Promise<void>;
};

export const PreferencesContext = createContext<PreferencesContextValue | null>(null);
