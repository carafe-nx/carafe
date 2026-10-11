import type { TFunction } from "i18next";
import type { BuildState } from "@/shared/api/bindings/BuildState";

export function busyReason(t: TFunction, build: BuildState | null, rebuildRunning: boolean): string | null {
  if (build?.state === "running") {
    return t("buildTask.busyBuild", { title: build.title });
  }
  if (rebuildRunning) {
    return t("buildTask.busyRebuild");
  }
  return null;
}
