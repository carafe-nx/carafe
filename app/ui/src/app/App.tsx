import { RouterProvider } from "react-router";
import { PreferencesProvider } from "@/features/preferences/PreferencesProvider";
import { router } from "./router";

export function App() {
  return (
    <PreferencesProvider>
      <RouterProvider router={router} />
    </PreferencesProvider>
  );
}
