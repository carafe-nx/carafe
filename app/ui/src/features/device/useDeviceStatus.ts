import { useEffect, useState } from "react";
import { api } from "@/shared/api/commands";
import type { DeviceStatus } from "@/shared/api/bindings/DeviceStatus";

const POLL_MS = 3000;

export function useDeviceStatus(): DeviceStatus {
  const [status, setStatus] = useState<DeviceStatus>({ connected: false, via: null });
  useEffect(() => {
    let active = true;
    const poll = () =>
      api.deviceStatus().then((next) => {
        if (active) {
          setStatus(next);
        }
      });
    void poll();
    const timer = window.setInterval(() => void poll(), POLL_MS);
    return () => {
      active = false;
      window.clearInterval(timer);
    };
  }, []);
  return status;
}
