import { createHashRouter } from "react-router";
import { LibraryPage } from "@/pages/library/LibraryPage";
import { OnboardingPage } from "@/pages/onboarding/OnboardingPage";
import { RebuildPage } from "@/pages/rebuild/RebuildPage";
import { SettingsPage } from "@/pages/settings/SettingsPage";
import { WizardPage } from "@/pages/wizard/WizardPage";
import { StartRedirect } from "./StartRedirect";

export const router = createHashRouter([
  { path: "/", element: <StartRedirect /> },
  { path: "/onboarding", element: <OnboardingPage /> },
  { path: "/library", element: <LibraryPage /> },
  { path: "/new", element: <WizardPage /> },
  { path: "/rebuild", element: <RebuildPage /> },
  { path: "/rebuild/:titleId", element: <WizardPage /> },
  { path: "/settings", element: <SettingsPage /> },
]);
