import { useEffect, useState } from "react";
import type { UpdateStatus } from "@/shared/api/bindings/UpdateStatus";
import { api } from "@/shared/api/commands";
import { useTauriEvent } from "@/shared/api/useTauriEvent";

export function useUpdateStatus(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  useEffect(() => {
    let active = true;
    void api.updateStatus().then((next) => {
      if (active) {
        setStatus((current) => current ?? next);
      }
    });
    return () => {
      active = false;
    };
  }, []);
  useTauriEvent("update://status", setStatus);
  return status;
}
