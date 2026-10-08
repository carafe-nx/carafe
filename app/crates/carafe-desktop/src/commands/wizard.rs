//! The wizard for creating and rebuilding a game.

use carafe_core::TitleId;
use carafe_core::library::{GameSummary, summaries};
use carafe_core::ports::{BuildProgress, PackTarget};
use carafe_core::wizard::{self, BuildRequest, Warning, WizardDraft};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use crate::error::CommandError;
use crate::services::{Services, entropy};

/// Build progress event.
pub const BUILD_PROGRESS: &str = "build://progress";
/// Build finished event.
pub const BUILD_FINISHED: &str = "build://finished";

/// Build result.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildFinished {
    /// Tile of the built game.
    pub game: Option<GameSummary>,
    /// The error, if the build failed.
    pub error: Option<CommandError>,
}

/// Inspects the game folder and returns the initial wizard state.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound` or `io` if the folder cannot be read.
#[tauri::command]
pub async fn inspect_folder(
    services: State<'_, Services>,
    folder: String,
) -> Result<WizardDraft, CommandError> {
    let report = services.inspector.inspect(&folder)?;
    Ok(wizard::draft(&report))
}

/// Returns the warnings for the selected `.exe`.
#[tauri::command]
pub async fn wizard_warnings(
    services: State<'_, Services>,
    draft: WizardDraft,
    executable: Option<String>,
) -> Result<Vec<Warning>, CommandError> {
    Ok(wizard::warnings(
        &draft,
        executable.as_deref(),
        services.capabilities,
    ))
}

/// Validates the request, starts the build and returns the Title ID; progress and the result arrive as
/// [`BUILD_PROGRESS`] and [`BUILD_FINISHED`] events.
///
/// # Errors
///
/// [`CommandError`] of kind `invalid` if the request fails the core validation or the settings lack
/// the path to `prod.keys` and the library folder.
#[tauri::command]
pub async fn start_build(
    app: AppHandle,
    mut request: BuildRequest,
) -> Result<TitleId, CommandError> {
    let services = app.state::<Services>();
    let icon = request.icon.take();
    let target = {
        let preferences = services.preferences();
        let keys_path = preferences
            .keys_path
            .clone()
            .ok_or_else(|| CommandError::invalid("no path to prod.keys set"))?;
        let library_dir = preferences
            .library_dir
            .clone()
            .ok_or_else(|| CommandError::invalid("no library folder set"))?;
        PackTarget {
            keys_path,
            library_dir,
        }
    };
    let previous = match request.title_id {
        Some(id) => services
            .store
            .list(&target.library_dir)?
            .into_iter()
            .find(|game| game.record.title_id == id)
            .map(|game| game.record),
        None => None,
    };
    let record = wizard::plan(
        request,
        previous.as_ref(),
        entropy(),
        &services.runtime_version,
    )?;
    let title_id = record.title_id;
    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let services = worker.state::<Services>();
        let mut report = |progress: BuildProgress| {
            let _ = worker.emit(BUILD_PROGRESS, progress);
        };
        let finished = match services
            .packer
            .pack(&record, icon.as_ref(), &target, &mut report)
        {
            Ok(game) => BuildFinished {
                game: summaries(&[game], &services.runtime_version).pop(),
                error: None,
            },
            Err(error) => BuildFinished {
                game: None,
                error: Some(error.into()),
            },
        };
        let _ = worker.emit(BUILD_FINISHED, finished);
    });
    Ok(title_id)
}
