import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import type { BuildFinished } from "./bindings/BuildFinished";
import type { BuildProgress } from "./bindings/BuildProgress";
import type { InstallEvent } from "./bindings/InstallEvent";
import type { RebuildStatus } from "./bindings/RebuildStatus";
import type { UpdateStatus } from "./bindings/UpdateStatus";

type EventMap = {
  "build://progress": BuildProgress;
  "build://finished": BuildFinished;
  "install://progress": InstallEvent;
  "install://finished": InstallEvent;
  "update://status": UpdateStatus;
  "rebuild://status": RebuildStatus;
};

export function useTauriEvent<K extends keyof EventMap>(name: K, handler: (payload: EventMap[K]) => void) {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => {
    let active = true;
    const pending = listen<EventMap[K]>(name, (event) => {
      if (active) {
        latest.current(event.payload);
      }
    });
    return () => {
      active = false;
      void pending.then((unlisten) => unlisten());
    };
  }, [name]);
}
