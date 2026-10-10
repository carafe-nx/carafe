import { useEffect, useState } from "react";
import type { RebuildStatus } from "@/shared/api/bindings/RebuildStatus";
import { api } from "@/shared/api/commands";
import { useTauriEvent } from "@/shared/api/useTauriEvent";

export function useRebuildStatus(): RebuildStatus | null {
  const [status, setStatus] = useState<RebuildStatus | null>(null);
  useEffect(() => {
    let active = true;
    void api.rebuildStatus().then((next) => {
      if (active) {
        setStatus((current) => current ?? next);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  useTauriEvent("rebuild://status", setStatus);
  return status;
}
