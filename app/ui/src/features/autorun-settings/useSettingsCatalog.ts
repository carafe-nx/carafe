import { useEffect, useState } from "react";
import { api } from "@/shared/api/commands";
import type { Catalog } from "@/shared/api/bindings/Catalog";

let loading: Promise<Catalog> | null = null;

export function useSettingsCatalog(): Catalog | null {
  const [catalog, setCatalog] = useState<Catalog | null>(null);

  useEffect(() => {
    let alive = true;
    loading ??= api.settingsCatalog();
    void loading.then((loaded) => {
      if (alive) {
        setCatalog(loaded);
      }
    });
    return () => {
      alive = false;
    };
  }, []);

  return catalog;
}
