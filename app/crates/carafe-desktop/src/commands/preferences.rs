//! Application settings and onboarding.

use std::path::Path;

use carafe_core::library;
use carafe_core::ports::{AdapterError, KeysReport};
use carafe_core::settings::AutorunSettings;
use tauri::State;

use crate::error::CommandError;
use crate::preferences::{self, Preferences};
use crate::services::Services;

/// Returns the application settings.
#[tauri::command]
pub async fn get_preferences(services: State<'_, Services>) -> Result<Preferences, CommandError> {
    Ok(services.preferences().clone())
}

/// Saves the application settings in memory and in the settings file.
///
/// If the system provided no settings folder, the settings last only until the application closes.
///
/// # Errors
///
/// [`CommandError`] of kind `invalid` if the default Autorun settings are invalid; of kind `io` if the
/// settings file cannot be written, in which case the settings in memory stay unchanged.
#[tauri::command]
pub async fn save_preferences(
    services: State<'_, Services>,
    preferences: Preferences,
) -> Result<Preferences, CommandError> {
    preferences
        .defaults
        .validate()
        .map_err(|error| CommandError::invalid(error.to_string()))?;
    if let Some(file) = services.preferences_file() {
        preferences::store(file, &preferences)
            .map_err(|error| AdapterError::Io(format!("{}: {error}", file.display())))?;
    }
    *services.preferences() = preferences.clone();
    Ok(preferences)
}

/// Returns the Autorun settings Carafe recommends for new games.
#[tauri::command]
pub async fn recommended_settings() -> AutorunSettings {
    AutorunSettings::default()
}

/// Checks whether the file contains the keys needed for packing.
///
/// # Errors
///
/// [`CommandError`] of kind `notFound` or `io` if the file cannot be read.
#[tauri::command]
pub async fn check_keys(
    services: State<'_, Services>,
    path: String,
) -> Result<KeysReport, CommandError> {
    Ok(services.keys.check(&path)?)
}

/// Returns the library folder for the folder the user picked: `Carafe` is appended to it
/// unless the folder is already named so.
#[tauri::command]
pub async fn library_dir_for(picked: String) -> String {
    library::library_dir_for(Path::new(&picked))
        .to_string_lossy()
        .into_owned()
}

/// Creates the library folder together with any missing parent folders; an existing folder
/// is left as is.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the folder cannot be created.
#[tauri::command]
pub async fn create_library_dir(path: String) -> Result<(), CommandError> {
    std::fs::create_dir_all(&path).map_err(|error| AdapterError::Io(format!("{path}: {error}")))?;
    Ok(())
}
