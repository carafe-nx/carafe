import { createHashRouter } from "react-router";
import { BuildPage } from "@/pages/build/BuildPage";
import { LibraryPage } from "@/pages/library/LibraryPage";
import { OnboardingPage } from "@/pages/onboarding/OnboardingPage";
import { RebuildPage } from "@/pages/rebuild/RebuildPage";
import { SettingsPage } from "@/pages/settings/SettingsPage";
import { WizardRoute } from "@/pages/wizard/WizardRoute";
import { StartRedirect } from "./StartRedirect";

export const router = createHashRouter([
  { path: "/", element: <StartRedirect /> },
  { path: "/onboarding", element: <OnboardingPage /> },
  { path: "/library", element: <LibraryPage /> },
  { path: "/build", element: <BuildPage /> },
  { path: "/new", element: <WizardRoute /> },
  { path: "/rebuild", element: <RebuildPage /> },
  { path: "/rebuild/:titleId", element: <WizardRoute /> },
  { path: "/settings", element: <SettingsPage /> },
]);
