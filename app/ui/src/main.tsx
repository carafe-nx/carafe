import "@fontsource-variable/inter";
import "@/shared/theme/tokens.css";
import "@/shared/theme/global.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/app/App";
import { initI18n } from "@/shared/i18n";

initI18n();

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
