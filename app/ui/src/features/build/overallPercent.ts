import type { BuildStage } from "@/shared/api/bindings/BuildStage";

export const BUILD_STAGES: BuildStage[] = ["runtime", "game", "metadata", "pack", "finish"];

export function overallPercent(stage: BuildStage, percent: number): number {
  return ((BUILD_STAGES.indexOf(stage) + percent / 100) / BUILD_STAGES.length) * 100;
}
