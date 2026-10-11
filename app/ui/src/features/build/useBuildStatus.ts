import { useEffect, useState } from "react";
import type { BuildState } from "@/shared/api/bindings/BuildState";
import { api } from "@/shared/api/commands";
import { useTauriEvent } from "@/shared/api/useTauriEvent";

export function useBuildStatus(): BuildState | null {
  const [status, setStatus] = useState<BuildState | null>(null);
  useEffect(() => {
    let active = true;
    void api.buildStatus().then((next) => {
      if (active) {
        setStatus((current) => current ?? next);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  useTauriEvent("build://status", setStatus);
  return status;
}
