//! Updating Carafe itself.

use tauri::{AppHandle, State};

use crate::error::CommandError;
use crate::updates::{self, UpdateStatus, Updates};

/// Returns where the update of Carafe stands; changes arrive as [`updates::UPDATE_STATUS`] events.
#[tauri::command]
pub async fn update_status(updates: State<'_, Updates>) -> Result<UpdateStatus, CommandError> {
    Ok(updates.status())
}

/// Looks for a newer Carafe at the player's request and returns the status after the check.
///
/// # Errors
///
/// [`CommandError`] of kind `network` if GitHub did not answer.
#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<UpdateStatus, CommandError> {
    updates::check(&app, true).await
}

/// Starts downloading the new version.
///
/// # Errors
///
/// [`CommandError`] of kind `network` if a check was needed and GitHub did not answer.
#[tauri::command]
pub async fn download_update(app: AppHandle) -> Result<(), CommandError> {
    updates::download(&app).await
}

/// Stops the running download.
#[tauri::command]
pub async fn cancel_update_download(app: AppHandle) -> Result<(), CommandError> {
    updates::cancel_download(&app);
    Ok(())
}

/// Hides the available version until a newer one is out.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the settings cannot be saved.
#[tauri::command]
pub async fn skip_update(app: AppHandle) -> Result<(), CommandError> {
    updates::skip(&app)
}

/// Installs the downloaded update now: Carafe closes and opens again on the new version.
///
/// # Errors
///
/// [`CommandError`] of kind `busy` while a build or a USB install is running, `notFound` or `io` if the
/// installer cannot be started.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), CommandError> {
    updates::install(&app, true)
}

/// Sets whether the downloaded update is installed when Carafe closes.
#[tauri::command]
pub async fn install_update_on_close(app: AppHandle, enabled: bool) -> Result<(), CommandError> {
    updates::set_install_on_close(&app, enabled);
    Ok(())
}

/// Sets whether the downloaded update is installed as soon as the running build or USB install finishes.
///
/// # Errors
///
/// [`CommandError`] as for [`install_update`] if nothing is running and it is installed at once.
#[tauri::command]
pub async fn install_update_when_idle(app: AppHandle, enabled: bool) -> Result<(), CommandError> {
    updates::set_install_when_idle(&app, enabled)
}
