import { useContext } from "react";
import { PreferencesContext, type PreferencesContextValue } from "./PreferencesContext";

export function usePreferences(): PreferencesContextValue {
  const value = useContext(PreferencesContext);
  if (!value) {
    throw new Error("usePreferences called outside PreferencesProvider");
  }
  return value;
}
