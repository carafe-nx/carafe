//! The "About" section.

use carafe_core::links::ExternalLink;

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
