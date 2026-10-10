//! The "About" section.

use carafe_core::links::{ExternalLink, release_url};
use carafe_core::version::Version;

use crate::error::CommandError;
use crate::shell;

/// Opens `link` in the system browser.
///
/// # Errors
///
/// [`CommandError`] if the browser failed to start.
#[tauri::command]
pub async fn open_link(link: ExternalLink) -> Result<(), CommandError> {
    shell::open_url(link.url()).map_err(|error| CommandError::invalid(error.to_string()))
}

/// Opens the release page of Carafe `version`, with its notes, in the system browser.
///
/// # Errors
///
/// [`CommandError`] of kind `invalid` if `version` is not a release version or the browser failed to
/// start.
#[tauri::command]
pub async fn open_release(version: String) -> Result<(), CommandError> {
    let version =
        Version::parse(&version).map_err(|error| CommandError::invalid(error.to_string()))?;
    shell::open_url(&release_url(version)).map_err(|error| CommandError::invalid(error.to_string()))
}
