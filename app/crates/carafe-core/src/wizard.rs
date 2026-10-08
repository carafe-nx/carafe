//! Rules of the game creation wizard: choosing the `.exe`, warnings, the build plan.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::icon::IconJpeg;
use crate::metadata::Metadata;
use crate::package::{PackageError, package};
use crate::ports::{ExecutableInfo, FolderReport};
use crate::record::{Arch, BuildRecord, FORMAT_VERSION, GameSource};
use crate::settings::{AutorunSettings, SettingsError};
use crate::title_id::TitleId;

/// Whether an `.exe` looks like the game itself or like an installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExecutableKind {
    /// Ordinary executable.
    Game,
    /// Installer or uninstaller: the name contains `setup`, `unins` or `unwise`.
    Installer,
}

/// Wizard warning. Does not prevent the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Warning {
    /// A 64-bit `.exe`, and the runtime does not yet run such games stably.
    UnstableX64,
    /// The folder has `steam_api.dll`: the game may need the Steam client.
    SteamApi,
    /// An installer or uninstaller is selected.
    InstallerSelected,
    /// The folder has only installers.
    OnlyInstallers,
}

/// What the current runtime can do; the warnings depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    /// 64-bit games run stably.
    pub stable_x64: bool,
}

/// An `.exe` in the wizard list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecutableChoice {
    /// Information about the file.
    pub info: ExecutableInfo,
    /// Game or installer.
    pub kind: ExecutableKind,
}

/// Initial wizard state after the folder is chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WizardDraft {
    /// Game folder.
    pub folder: String,
    /// `.exe` files: games first by descending size, then installers.
    pub executables: Vec<ExecutableChoice>,
    /// The `.exe` selected by default; `None` if the folder has only installers.
    pub selected: Option<String>,
    /// Default metadata.
    pub metadata: Metadata,
    /// The folder has `steam_api.dll`.
    pub has_steam_api: bool,
}

/// Returns whether a file looks like an installer: its name contains `setup`, `unins` or `unwise`
/// (the uninstaller of Wise installers).
#[must_use]
pub fn classify(path: &str) -> ExecutableKind {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    if ["setup", "unins", "unwise"]
        .iter()
        .any(|marker| name.contains(marker))
    {
        ExecutableKind::Installer
    } else {
        ExecutableKind::Game
    }
}

/// Composes the initial wizard state from the folder scan.
///
/// The default title is the `ProductName` of the selected `.exe`, otherwise the folder name; the publisher is "Carafe".
#[must_use]
pub fn draft(report: &FolderReport) -> WizardDraft {
    let mut executables: Vec<ExecutableChoice> = report
        .executables
        .iter()
        .map(|info| ExecutableChoice {
            info: info.clone(),
            kind: classify(&info.path),
        })
        .collect();
    executables.sort_by(|a, b| {
        let a_installer = a.kind == ExecutableKind::Installer;
        let b_installer = b.kind == ExecutableKind::Installer;
        a_installer
            .cmp(&b_installer)
            .then(b.info.size_bytes.cmp(&a.info.size_bytes))
    });
    let default = executables
        .iter()
        .find(|choice| choice.kind == ExecutableKind::Game);
    let folder_name = report
        .folder
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(&report.folder);
    let title = default
        .and_then(|choice| choice.info.product_name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| folder_name.to_owned());
    let selected = default.map(|choice| choice.info.path.clone());
    WizardDraft {
        folder: report.folder.clone(),
        executables,
        selected,
        metadata: Metadata::new(title),
        has_steam_api: report.has_steam_api,
    }
}

/// Returns the warnings for the selected `.exe`.
#[must_use]
pub fn warnings(
    draft: &WizardDraft,
    executable: Option<&str>,
    runtime: RuntimeCapabilities,
) -> Vec<Warning> {
    let mut found = Vec::new();
    let only_installers = draft
        .executables
        .iter()
        .all(|choice| choice.kind == ExecutableKind::Installer);
    if only_installers {
        found.push(Warning::OnlyInstallers);
    }
    let choice = executable.and_then(|path| {
        draft
            .executables
            .iter()
            .find(|choice| choice.info.path == path)
    });
    if let Some(choice) = choice {
        if choice.kind == ExecutableKind::Installer && !only_installers {
            found.push(Warning::InstallerSelected);
        }
        if choice.info.arch == Arch::X64 && !runtime.stable_x64 {
            found.push(Warning::UnstableX64);
        }
    }
    if draft.has_steam_api {
        found.push(Warning::SteamApi);
    }
    found
}

/// What the window submits for a build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildRequest {
    /// Title ID of the game being rebuilt; `None` means a new game.
    pub title_id: Option<TitleId>,
    /// What to build from.
    pub source: GameSource,
    /// Metadata.
    pub metadata: Metadata,
    /// Autorun settings.
    pub settings: AutorunSettings,
    /// Icon the window made from the chosen source; `None` means the default runtime icon.
    #[ts(type = "number[] | null")]
    pub icon: Option<IconJpeg>,
}

/// Build plan error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanError {
    /// Empty title.
    #[error("the game has no title")]
    EmptyTitle,
    /// No `.exe` selected.
    #[error("no executable selected")]
    NoExecutable,
    /// An icon source is chosen, but the image itself is not.
    #[error("no image selected for the icon")]
    NoIconImage,
    /// Rebuild of a game that is not in the library.
    #[error("game {0} is not in the library")]
    UnknownGame(TitleId),
    /// Invalid settings.
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// The folder, `.exe` or arguments cannot be passed to the runtime.
    #[error(transparent)]
    Package(#[from] PackageError),
}

/// Composes the build record.
///
/// A new game gets a Title ID from `entropy` and build number 1; a rebuild gets the Title ID and the next number
/// from `previous`.
///
/// # Errors
///
/// [`PlanError`] if the title is empty, the `.exe` or the icon image is not selected, the settings
/// are invalid, the folder, `.exe` or arguments cannot be passed to the runtime, or a game is rebuilt
/// without the `previous` record.
pub fn plan(
    request: BuildRequest,
    previous: Option<&BuildRecord>,
    entropy: u64,
    runtime_version: &str,
) -> Result<BuildRecord, PlanError> {
    if request.metadata.title.trim().is_empty() {
        return Err(PlanError::EmptyTitle);
    }
    if request.source.executable.trim().is_empty() {
        return Err(PlanError::NoExecutable);
    }
    if !request.metadata.icon.is_chosen() {
        return Err(PlanError::NoIconImage);
    }
    request.settings.validate()?;
    let (title_id, build_number) = match (request.title_id, previous) {
        (None, _) => (TitleId::from_entropy(entropy), 1),
        (Some(id), Some(previous)) if previous.title_id == id => (id, previous.build_number + 1),
        (Some(id), _) => return Err(PlanError::UnknownGame(id)),
    };
    let record = BuildRecord {
        format_version: FORMAT_VERSION,
        title_id,
        build_number,
        runtime_version: runtime_version.to_owned(),
        source: request.source,
        metadata: request.metadata,
        settings: request.settings,
    };
    package(&record)?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{Crop, IconSource};

    const STABLE: RuntimeCapabilities = RuntimeCapabilities { stable_x64: true };
    const TODAY: RuntimeCapabilities = RuntimeCapabilities { stable_x64: false };

    fn exe(path: &str, arch: Arch, size_bytes: u64) -> ExecutableInfo {
        ExecutableInfo {
            path: path.to_owned(),
            arch,
            size_bytes,
            product_name: None,
            company_name: None,
        }
    }

    fn report(executables: Vec<ExecutableInfo>) -> FolderReport {
        FolderReport {
            folder: "D:\\Game\\openttd-15.3-windows-win32\\".to_owned(),
            executables,
            has_steam_api: false,
        }
    }

    #[test]
    fn installers_classified_by_name() {
        assert_eq!(classify("unins000.exe"), ExecutableKind::Installer);
        assert_eq!(classify("bin\\Setup.exe"), ExecutableKind::Installer);
        assert_eq!(classify("3DSetup\\3DSetup.exe"), ExecutableKind::Installer);
        assert_eq!(classify("eauninstall.exe"), ExecutableKind::Installer);
        assert_eq!(classify("UNWISE.EXE"), ExecutableKind::Installer);
        assert_eq!(classify("openttd.exe"), ExecutableKind::Game);
        assert_eq!(classify("Speed.exe"), ExecutableKind::Game);
    }

    #[test]
    fn largest_game_selected_and_installers_last() {
        let draft = draft(&report(vec![
            exe("unins000.exe", Arch::X86, 30_000_000),
            exe("openttd.exe", Arch::X86, 8_000_000),
            exe("helper.exe", Arch::X86, 100_000),
        ]));
        assert_eq!(draft.selected.as_deref(), Some("openttd.exe"));
        assert_eq!(draft.executables.last().unwrap().info.path, "unins000.exe");
        assert_eq!(draft.metadata.title, "openttd-15.3-windows-win32");
    }

    #[test]
    fn product_name_of_the_selected_exe_wins_over_folder_name() {
        let mut game = exe("openttd.exe", Arch::X86, 8);
        game.product_name = Some("OpenTTD".to_owned());
        let mut helper = exe("crashreport.exe", Arch::X86, 1);
        helper.product_name = Some("Crash Reporter".to_owned());
        let folder = report(vec![helper, game]);
        assert_eq!(draft(&folder).metadata.title, "OpenTTD");
    }

    #[test]
    fn blank_product_name_falls_back_to_folder_name() {
        let mut game = exe("openttd.exe", Arch::X86, 8);
        game.product_name = Some("  ".to_owned());
        let folder = report(vec![game]);
        assert_eq!(draft(&folder).metadata.title, "openttd-15.3-windows-win32");
    }

    #[test]
    fn x64_warning_depends_on_runtime() {
        let draft = draft(&report(vec![exe("game.exe", Arch::X64, 1)]));
        assert_eq!(
            warnings(&draft, Some("game.exe"), TODAY),
            vec![Warning::UnstableX64]
        );
        assert!(warnings(&draft, Some("game.exe"), STABLE).is_empty());
    }

    #[test]
    fn only_installers_selects_nothing() {
        let draft = draft(&report(vec![exe("setup.exe", Arch::X86, 1)]));
        assert_eq!(draft.selected, None);
        assert_eq!(
            warnings(&draft, None, STABLE),
            vec![Warning::OnlyInstallers]
        );
    }

    #[test]
    fn steam_api_warns() {
        let mut folder = report(vec![exe("game.exe", Arch::X86, 1)]);
        folder.has_steam_api = true;
        let draft = draft(&folder);
        assert_eq!(
            warnings(&draft, Some("game.exe"), STABLE),
            vec![Warning::SteamApi]
        );
    }

    fn request(title_id: Option<TitleId>) -> BuildRequest {
        BuildRequest {
            title_id,
            source: GameSource {
                folder: "D:\\Game\\openttd".to_owned(),
                executable: "openttd.exe".to_owned(),
                arch: Arch::X86,
                arguments: Vec::new(),
            },
            metadata: Metadata::new("OpenTTD"),
            settings: AutorunSettings::default(),
            icon: None,
        }
    }

    #[test]
    fn new_game_gets_fresh_id_and_first_build() {
        let record = plan(request(None), None, 42, "0.1.0").unwrap();
        assert_eq!(record.title_id, TitleId::from_entropy(42));
        assert_eq!(record.build_number, 1);
    }

    #[test]
    fn rebuild_keeps_id_and_counts_up() {
        let first = plan(request(None), None, 42, "0.1.0").unwrap();
        let second = plan(request(Some(first.title_id)), Some(&first), 7, "0.2.0").unwrap();
        assert_eq!(second.title_id, first.title_id);
        assert_eq!(second.build_number, 2);
        assert_eq!(second.runtime_version, "0.2.0");
    }

    #[test]
    fn rebuild_without_previous_record_fails() {
        let id = TitleId::from_entropy(1);
        assert_eq!(
            plan(request(Some(id)), None, 0, "0.1.0"),
            Err(PlanError::UnknownGame(id))
        );
    }

    #[test]
    fn executable_outside_the_folder_rejected() {
        let mut outside = request(None);
        outside.source.executable = "..\\other.exe".to_owned();
        assert!(matches!(
            plan(outside, None, 0, "0.1.0"),
            Err(PlanError::Package(PackageError::ExecutableOutside(_)))
        ));
    }

    #[test]
    fn empty_title_rejected() {
        let mut empty = request(None);
        empty.metadata.title = " ".to_owned();
        assert_eq!(plan(empty, None, 0, "0.1.0"), Err(PlanError::EmptyTitle));
    }

    #[test]
    fn icon_source_without_image_rejected() {
        let mut blank = request(None);
        blank.metadata.icon = IconSource::SteamGridDb {
            game_id: 0,
            image_id: 0,
            url: String::new(),
            crop: Crop {
                x: 0,
                y: 0,
                size: 0,
            },
        };
        assert_eq!(plan(blank, None, 0, "0.1.0"), Err(PlanError::NoIconImage));
    }
}
