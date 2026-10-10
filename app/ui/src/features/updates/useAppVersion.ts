import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";

export function useAppVersion(): string | null {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    void getVersion().then((next) => {
      if (active) {
        setVersion(next);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  return version;
}
