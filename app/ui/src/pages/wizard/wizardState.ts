import type { AutorunSettings } from "@/shared/api/bindings/AutorunSettings";
import type { BuildRecord } from "@/shared/api/bindings/BuildRecord";
import type { BuildRequest } from "@/shared/api/bindings/BuildRequest";
import type { IconSource } from "@/shared/api/bindings/IconSource";
import type { Metadata } from "@/shared/api/bindings/Metadata";
import type { TitleId } from "@/shared/api/bindings/TitleId";
import type { WizardDraft } from "@/shared/api/bindings/WizardDraft";

export const STEPS = ["folder", "launch", "look", "controls", "graphics", "build"] as const;
export type StepId = (typeof STEPS)[number];

export type WizardState = {
  step: StepId;
  reached: number;
  buildNumber: number;
  titleId: TitleId | null;
  draft: WizardDraft | null;
  executable: string | null;
  args: string;
  metadata: Metadata;
  settings: AutorunSettings;
};

export type WizardAction =
  | { type: "goTo"; step: StepId }
  | { type: "draftLoaded"; draft: WizardDraft }
  | { type: "recordLoaded"; record: BuildRecord; draft: WizardDraft | null }
  | { type: "executable"; path: string }
  | { type: "args"; args: string }
  | { type: "metadata"; metadata: Metadata }
  | { type: "settings"; settings: AutorunSettings };

export function initialState(defaults: AutorunSettings): WizardState {
  return {
    step: "folder",
    reached: 0,
    buildNumber: 1,
    titleId: null,
    draft: null,
    executable: null,
    args: "",
    metadata: { title: "", publisher: "Carafe", localized: [], displayVersion: null, icon: { kind: "executable" } },
    settings: defaults,
  };
}

export function wizardReducer(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case "goTo": {
      const index = STEPS.indexOf(action.step);
      return { ...state, step: action.step, reached: Math.max(state.reached, index) };
    }
    case "draftLoaded": {
      const rebuild = state.titleId !== null;
      const keepExecutable = rebuild && action.draft.executables.some((choice) => choice.info.path === state.executable);
      return {
        ...state,
        draft: action.draft,
        executable: keepExecutable ? state.executable : action.draft.selected,
        metadata: rebuild ? state.metadata : action.draft.metadata,
      };
    }
    case "recordLoaded":
      return {
        ...state,
        step: action.draft ? state.step : "folder",
        reached: action.draft ? STEPS.length - 1 : 0,
        buildNumber: action.record.buildNumber + 1,
        titleId: action.record.titleId,
        draft: action.draft,
        executable: action.record.source.executable,
        args: action.record.source.arguments.join(" "),
        metadata: action.record.metadata,
        settings: action.record.settings,
      };
    case "executable":
      return { ...state, executable: action.path };
    case "args":
      return { ...state, args: action.args };
    case "metadata":
      return { ...state, metadata: action.metadata };
    case "settings":
      return { ...state, settings: action.settings };
  }
}

export function iconChosen(icon: IconSource): boolean {
  switch (icon.kind) {
    case "executable":
      return true;
    case "file":
      return icon.path.trim() !== "";
    case "steamGridDb":
      return icon.url.trim() !== "";
  }
}

export function canLeave(state: WizardState, step: StepId): boolean {
  switch (step) {
    case "folder":
      return state.draft !== null;
    case "launch":
      return state.executable !== null;
    case "look":
      return state.metadata.title.trim() !== "" && iconChosen(state.metadata.icon);
    default:
      return true;
  }
}

export function canReach(state: WizardState, step: StepId): boolean {
  return STEPS.slice(0, STEPS.indexOf(step)).every((before) => canLeave(state, before));
}

export function toRequest(state: WizardState): BuildRequest | null {
  const choice = state.draft?.executables.find((item) => item.info.path === state.executable);
  if (!state.draft || !choice || !canReach(state, "build")) {
    return null;
  }
  return {
    titleId: state.titleId,
    source: {
      folder: state.draft.folder,
      executable: choice.info.path,
      arch: choice.info.arch,
      arguments: state.args.split(" ").filter((part) => part !== ""),
    },
    metadata: state.metadata,
    settings: state.settings,
    icon: null,
  };
}
