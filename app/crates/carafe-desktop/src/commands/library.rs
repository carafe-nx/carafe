//! The library: tiles, deletion, installation to the Switch, logs.

use std::path::{Path, PathBuf};

use carafe_core::TitleId;
use carafe_core::dbi::InstallTarget;
use carafe_core::library::{GameSummary, summaries};
use carafe_core::ports::{AdapterError, StoredGame, TransferProgress};
use carafe_core::record::BuildRecord;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use crate::error::CommandError;
use crate::services::Services;
use crate::shell;

/// Folder for console logs inside the library folder.
pub const LOGS_DIR: [&str; 2] = [".carafe", "logs"];

/// Installation progress event.
pub const INSTALL_PROGRESS: &str = "install://progress";
/// Installation finished event.
pub const INSTALL_FINISHED: &str = "install://finished";

/// Installation progress of one game.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallEvent {
    /// The game being installed.
    pub title_id: TitleId,
    /// How much has been transferred.
    pub progress: Option<TransferProgress>,
    /// The error, if the installation failed.
    pub error: Option<CommandError>,
}

/// Returns the library tiles.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the library folder is unreadable.
#[tauri::command]
pub async fn list_games(services: State<'_, Services>) -> Result<Vec<GameSummary>, CommandError> {
    let games = services.games()?;
    Ok(summaries(&games, &services.runtime_version))
}

/// Returns the build record of a game, for "Rebuild".
///
/// # Errors
///
/// [`CommandError`] of kind `notFound` if the game does not exist.
#[tauri::command]
pub async fn get_record(
    services: State<'_, Services>,
    title_id: TitleId,
) -> Result<BuildRecord, CommandError> {
    Ok(find(&services, title_id)?.record)
}

/// Deletes the game's NSP from the PC.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound` or `io`.
#[tauri::command]
pub async fn delete_game(
    services: State<'_, Services>,
    title_id: TitleId,
) -> Result<(), CommandError> {
    let dir = services
        .library_dir()
        .ok_or_else(|| AdapterError::NotFound(title_id.to_string()))?;
    Ok(services.store.delete(&dir, title_id)?)
}

/// Starts installation to the Switch into `target`; progress and the result arrive as [`INSTALL_PROGRESS`]
/// and [`INSTALL_FINISHED`] events.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound` if the game does not exist.
#[tauri::command]
pub async fn install_game(
    app: AppHandle,
    title_id: TitleId,
    target: InstallTarget,
) -> Result<(), CommandError> {
    let services = app.state::<Services>();
    let game = find(&services, title_id)?;
    let dir = services
        .library_dir()
        .ok_or_else(|| AdapterError::NotFound(title_id.to_string()))?;
    let nsp = Path::new(&dir).join(&game.file_name);
    let nsp = nsp.to_string_lossy().into_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let services = app.state::<Services>();
        let mut report = |progress: TransferProgress| {
            let event = InstallEvent {
                title_id,
                progress: Some(progress),
                error: None,
            };
            let _ = app.emit(INSTALL_PROGRESS, event);
        };
        let result = services.device.install(&nsp, target, &mut report);
        let event = InstallEvent {
            title_id,
            progress: None,
            error: result.err().map(CommandError::from),
        };
        let _ = app.emit(INSTALL_FINISHED, event);
    });
    Ok(())
}

/// Copies the game's logs from the console's memory card to the library's `.carafe/logs/<Title ID>`,
/// opens that folder in the file manager and returns its path.
///
/// # Errors
///
/// [`CommandError`] of kind `device`, `notFound` or `io`.
#[tauri::command]
pub async fn fetch_logs(app: AppHandle, title_id: TitleId) -> Result<String, CommandError> {
    let dir = app
        .state::<Services>()
        .library_dir()
        .ok_or_else(|| AdapterError::NotFound(title_id.to_string()))?;
    let dest = LOGS_DIR
        .iter()
        .fold(PathBuf::from(dir), |path, part| path.join(part))
        .join(title_id.to_string());
    let dest_text = dest.to_string_lossy().into_owned();
    let copied = dest_text.clone();
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Services>().device.fetch_logs(title_id, &copied)
    })
    .await
    .map_err(|error| CommandError::invalid(error.to_string()))??;
    let _ = shell::open_folder(&dest);
    Ok(dest_text)
}

fn find(services: &Services, title_id: TitleId) -> Result<StoredGame, CommandError> {
    services
        .games()?
        .into_iter()
        .find(|game| game.record.title_id == title_id)
        .ok_or_else(|| AdapterError::NotFound(title_id.to_string()).into())
}
