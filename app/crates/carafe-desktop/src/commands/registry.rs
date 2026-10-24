//! The game's registry: importing and exporting `.reg` files.

use carafe_core::ports::AdapterError;
use carafe_core::registry::{RegImport, RegistryEntry, export_reg, parse_reg};
use tauri::{AppHandle, Manager};

use crate::error::CommandError;
use crate::services::Services;

/// Reads a `.reg` file and returns its entries and the lines that were left out.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound` or `io` if the file cannot be read; `unsupported` if it is too large or is
/// not a registry file.
#[tauri::command]
pub async fn import_registry(app: AppHandle, path: String) -> Result<RegImport, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = app.state::<Services>().registry_files.read(&path)?;
        parse_reg(&bytes).map_err(|error| AdapterError::Unsupported(format!("{path}: {error}")))
    })
    .await
    .map_err(|error| CommandError::from(AdapterError::Io(error.to_string())))?
    .map_err(CommandError::from)
}

/// Writes the entries into a `.reg` file the way Windows' `regedit` exports them.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the file cannot be written.
#[tauri::command]
pub async fn export_registry(
    app: AppHandle,
    path: String,
    entries: Vec<RegistryEntry>,
) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Services>()
            .registry_files
            .write(&path, &export_reg(&entries))
    })
    .await
    .map_err(|error| CommandError::from(AdapterError::Io(error.to_string())))?
    .map_err(CommandError::from)
}
