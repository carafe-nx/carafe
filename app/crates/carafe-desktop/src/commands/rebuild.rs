//! Rebuilding games made with an older runtime.

use carafe_core::TitleId;
use carafe_core::library::summaries;
use carafe_core::rebuild::{RebuildOffer, rebuild_offer, rebuild_space};
use tauri::{AppHandle, Manager, State};

use crate::error::{CommandError, ErrorCode};
use crate::preferences;
use crate::rebuilds::{self, RebuildStatus, Rebuilds};
use crate::services::Services;

/// Returns the library's offer to rebuild games made with an older runtime.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the library folder is unreadable.
#[tauri::command]
pub async fn rebuild_offer_status(
    services: State<'_, Services>,
) -> Result<RebuildOffer, CommandError> {
    let games = summaries(&services.games()?, &services.runtime_version);
    let dismissed = services.preferences().rebuild_dismissed.clone();
    Ok(rebuild_offer(
        &games,
        &services.runtime_version,
        dismissed.as_deref(),
    ))
}

/// Stops offering a rebuild until Carafe carries a newer runtime.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the settings cannot be saved.
#[tauri::command]
pub async fn dismiss_rebuild_offer(services: State<'_, Services>) -> Result<(), CommandError> {
    let mut next = services.preferences().clone();
    next.rebuild_dismissed = Some(services.runtime_version.clone());
    if let Some(file) = services.preferences_file() {
        preferences::store(file, &next).map_err(|error| CommandError {
            code: ErrorCode::Io,
            message: error.to_string(),
        })?;
    }
    *services.preferences() = next;
    Ok(())
}

/// Returns how many bytes of free space rebuilding `games` needs.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the library folder is unreadable.
#[tauri::command]
pub async fn rebuild_space_needed(
    services: State<'_, Services>,
    games: Vec<TitleId>,
) -> Result<u64, CommandError> {
    let sizes: Vec<u64> = services
        .games()?
        .iter()
        .filter(|game| games.contains(&game.record.title_id))
        .map(|game| game.size_bytes)
        .collect();
    Ok(rebuild_space(&sizes))
}

/// Starts rebuilding `games` one by one; progress arrives as [`rebuilds::REBUILD_STATUS`] events.
///
/// # Errors
///
/// [`CommandError`] of kind `busy` if a rebuild is already running, `invalid` if the settings are
/// incomplete.
#[tauri::command]
pub async fn start_rebuild(app: AppHandle, games: Vec<TitleId>) -> Result<(), CommandError> {
    rebuilds::start(&app, games)
}

/// Asks the running rebuild to stop after the current game.
#[tauri::command]
pub async fn stop_rebuild(app: AppHandle) -> Result<(), CommandError> {
    rebuilds::stop(&app);
    Ok(())
}

/// Returns where the rebuild stands.
#[tauri::command]
pub async fn rebuild_status(app: AppHandle) -> Result<RebuildStatus, CommandError> {
    Ok(app.state::<Rebuilds>().status())
}
