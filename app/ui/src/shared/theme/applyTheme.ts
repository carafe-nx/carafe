import type { Theme } from "@/shared/api/bindings/Theme";

const darkQuery = "(prefers-color-scheme: dark)";

export function resolveTheme(theme: Theme): "light" | "dark" {
  if (theme === "system") {
    return window.matchMedia(darkQuery).matches ? "dark" : "light";
  }
  return theme;
}

export function applyTheme(theme: Theme): () => void {
  const apply = () => document.documentElement.setAttribute("data-theme", resolveTheme(theme));
  apply();
  if (theme !== "system") {
    return () => {};
  }
  const media = window.matchMedia(darkQuery);
  media.addEventListener("change", apply);
  return () => media.removeEventListener("change", apply);
}
