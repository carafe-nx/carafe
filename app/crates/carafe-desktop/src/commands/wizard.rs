//! The wizard for creating and rebuilding a game.

use std::time::{Duration, Instant};

use carafe_core::TitleId;
use carafe_core::library::{GameSummary, summaries};
use carafe_core::ports::{BuildProgress, PackTarget};
use carafe_core::record::Arch;
use carafe_core::wizard::{self, BuildRequest, Warning, WizardDraft};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use crate::builds::{self, BuildState, Builds};
use crate::error::CommandError;
use crate::services::{Services, entropy};
use crate::updates;

const PROGRESS_STEP: Duration = Duration::from_millis(200);

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

/// Returns the warnings for the selected `.exe` and the bitness chosen by hand.
#[tauri::command]
pub async fn wizard_warnings(
    services: State<'_, Services>,
    draft: WizardDraft,
    executable: Option<String>,
    arch_override: Option<Arch>,
) -> Result<Vec<Warning>, CommandError> {
    Ok(wizard::warnings(
        &draft,
        executable.as_deref(),
        arch_override,
        services.capabilities,
    ))
}

/// Validates the request, starts the build and returns the Title ID; progress and the result arrive as
/// [`BUILD_PROGRESS`] and [`BUILD_FINISHED`] events.
///
/// # Errors
///
/// [`CommandError`] of kind `invalid` if the request fails the core validation or the settings lack
/// the path to `prod.keys` and the library folder; `busy` if a build or a rebuild is running.
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
    let cancel = builds::start(&app, title_id, record.metadata.title.clone())?;
    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _work = updates::begin_work(&worker);
        let services = worker.state::<Services>();
        let mut shown: Option<Instant> = None;
        let mut report = |progress: BuildProgress| {
            if shown.is_none_or(|at| at.elapsed() >= PROGRESS_STEP) {
                shown = Some(Instant::now());
                builds::progress(&worker, progress.stage, progress.percent);
            }
            let _ = worker.emit(BUILD_PROGRESS, progress);
        };
        let result = services
            .packer
            .pack(&record, icon.as_ref(), &target, &cancel, &mut report);
        let finished = match &result {
            Ok(game) => BuildFinished {
                game: summaries(std::slice::from_ref(game), &services.runtime_version).pop(),
                error: None,
            },
            Err(error) => BuildFinished {
                game: None,
                error: Some(error.clone().into()),
            },
        };
        builds::finish(&worker, result.map(|_| ()));
        let _ = worker.emit(BUILD_FINISHED, finished);
    });
    Ok(title_id)
}

/// Returns the state of the wizard's build.
#[tauri::command]
pub async fn build_status(app: AppHandle) -> Result<BuildState, CommandError> {
    Ok(app.state::<Builds>().state())
}

/// Asks the running build to stop; it ends as cancelled once it has stopped and cleaned up.
#[tauri::command]
pub async fn cancel_build(app: AppHandle) -> Result<(), CommandError> {
    builds::cancel(&app);
    Ok(())
}

/// Closes the result of a finished build.
#[tauri::command]
pub async fn dismiss_build(app: AppHandle) -> Result<(), CommandError> {
    builds::dismiss(&app);
    Ok(())
}
