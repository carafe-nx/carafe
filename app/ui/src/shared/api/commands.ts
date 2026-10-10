import { invoke } from "@tauri-apps/api/core";
import type { ArtGame } from "./bindings/ArtGame";
import type { ArtImage } from "./bindings/ArtImage";
import type { AutorunSettings } from "./bindings/AutorunSettings";
import type { BuildRecord } from "./bindings/BuildRecord";
import type { BuildRequest } from "./bindings/BuildRequest";
import type { CommandError } from "./bindings/CommandError";
import type { DeviceStatus } from "./bindings/DeviceStatus";
import type { ExternalLink } from "./bindings/ExternalLink";
import type { GameSummary } from "./bindings/GameSummary";
import type { InstallTarget } from "./bindings/InstallTarget";
import type { KeysReport } from "./bindings/KeysReport";
import type { Preferences } from "./bindings/Preferences";
import type { RebuildOffer } from "./bindings/RebuildOffer";
import type { RebuildStatus } from "./bindings/RebuildStatus";
import type { TitleId } from "./bindings/TitleId";
import type { UpdateStatus } from "./bindings/UpdateStatus";
import type { Warning } from "./bindings/Warning";
import type { WizardDraft } from "./bindings/WizardDraft";

export const api = {
  getPreferences: () => invoke<Preferences>("get_preferences"),
  savePreferences: (preferences: Preferences) => invoke<Preferences>("save_preferences", { preferences }),
  checkKeys: (path: string) => invoke<KeysReport>("check_keys", { path }),
  recommendedSettings: () => invoke<AutorunSettings>("recommended_settings"),
  libraryDirFor: (picked: string) => invoke<string>("library_dir_for", { picked }),
  createLibraryDir: (path: string) => invoke<void>("create_library_dir", { path }),
  deviceStatus: () => invoke<DeviceStatus>("device_status"),
  listGames: () => invoke<GameSummary[]>("list_games"),
  getRecord: (titleId: TitleId) => invoke<BuildRecord>("get_record", { titleId }),
  deleteGame: (titleId: TitleId) => invoke<void>("delete_game", { titleId }),
  installGame: (titleId: TitleId, target: InstallTarget) => invoke<void>("install_game", { titleId, target }),
  fetchLogs: (titleId: TitleId) => invoke<string>("fetch_logs", { titleId }),
  inspectFolder: (folder: string) => invoke<WizardDraft>("inspect_folder", { folder }),
  wizardWarnings: (draft: WizardDraft, executable: string | null) =>
    invoke<Warning[]>("wizard_warnings", { draft, executable }),
  startBuild: (request: BuildRequest) => invoke<TitleId>("start_build", { request }),
  readImage: (path: string) => invoke<string>("read_image", { path }),
  executableIcon: (folder: string, executable: string) =>
    invoke<string | null>("executable_icon", { folder, executable }),
  artGames: (term: string) => invoke<ArtGame[]>("art_games", { term }),
  artImages: (gameId: number) => invoke<ArtImage[]>("art_images", { gameId }),
  artDownload: (url: string) => invoke<string>("art_download", { url }),
  openLink: (link: ExternalLink) => invoke<void>("open_link", { link }),
  openRelease: (version: string) => invoke<void>("open_release", { version }),
  updateStatus: () => invoke<UpdateStatus>("update_status"),
  checkUpdates: () => invoke<UpdateStatus>("check_updates"),
  downloadUpdate: () => invoke<void>("download_update"),
  cancelUpdateDownload: () => invoke<void>("cancel_update_download"),
  skipUpdate: () => invoke<void>("skip_update"),
  installUpdate: () => invoke<void>("install_update"),
  installUpdateOnClose: (enabled: boolean) => invoke<void>("install_update_on_close", { enabled }),
  installUpdateWhenIdle: (enabled: boolean) => invoke<void>("install_update_when_idle", { enabled }),
  rebuildOffer: () => invoke<RebuildOffer>("rebuild_offer_status"),
  dismissRebuildOffer: () => invoke<void>("dismiss_rebuild_offer"),
  rebuildSpaceNeeded: (games: TitleId[]) => invoke<number>("rebuild_space_needed", { games }),
  startRebuild: (games: TitleId[]) => invoke<void>("start_rebuild", { games }),
  stopRebuild: () => invoke<void>("stop_rebuild"),
  rebuildStatus: () => invoke<RebuildStatus>("rebuild_status"),
};

export function isCommandError(value: unknown): value is CommandError {
  return typeof value === "object" && value !== null && "code" in value && "message" in value;
}
