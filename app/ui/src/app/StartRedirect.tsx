import { Navigate } from "react-router";
import { usePreferences } from "@/features/preferences/usePreferences";

export function StartRedirect() {
  const { preferences } = usePreferences();
  return <Navigate to={preferences.onboarded ? "/library" : "/onboarding"} replace />;
}
