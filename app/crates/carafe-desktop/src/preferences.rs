//! Settings of the application itself and their file.

use std::fs;
use std::io;
use std::path::Path;

use carafe_core::settings::AutorunSettings;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Name of the settings file in the application settings folder.
pub const PREFERENCES_FILE: &str = "preferences.json";

/// Interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Language {
    /// Same as the system.
    #[default]
    System,
    /// Russian.
    Ru,
    /// English.
    En,
}

/// Interface theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    /// Same as the system.
    #[default]
    System,
    /// Light.
    Light,
    /// Dark.
    Dark,
}

/// Application settings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Preferences {
    /// Onboarding is complete.
    pub onboarded: bool,
    /// Interface language.
    pub language: Language,
    /// Interface theme.
    pub theme: Theme,
    /// Path to `prod.keys`.
    pub keys_path: Option<String>,
    /// Library folder.
    pub library_dir: Option<String>,
    /// Autorun settings for new games.
    pub defaults: AutorunSettings,
    /// SteamGridDB API key for cover search.
    pub steam_grid_db_key: Option<String>,
}

/// Returns the settings from the file.
///
/// Returns `None` if the file is missing, unreadable or cannot be parsed: the application then starts
/// with the default settings. Fields missing from the file are taken from [`Preferences::default`].
pub fn load(path: &Path) -> Option<Preferences> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Writes the settings to the file, creating the folder if needed.
///
/// First writes a temporary file next to it and renames it over the old one, so a failure mid-write
/// does not leave half a file.
///
/// # Errors
///
/// [`io::Error`] if the folder cannot be created or the file cannot be written.
pub fn store(path: &Path, preferences: &Preferences) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_vec_pretty(preferences).map_err(io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("carafe-preferences-{}-{name}", std::process::id()))
    }

    #[test]
    fn stored_preferences_load_back() {
        let dir = temp_dir("roundtrip");
        let path = dir.join(PREFERENCES_FILE);
        let preferences = Preferences {
            onboarded: true,
            language: Language::Ru,
            theme: Theme::Dark,
            keys_path: Some("D:/keys/prod.keys".to_owned()),
            library_dir: Some("D:/Carafe".to_owned()),
            defaults: AutorunSettings::default(),
            steam_grid_db_key: Some("0123abcd".to_owned()),
        };
        store(&path, &preferences).expect("stored");
        let loaded = load(&path);
        fs::remove_dir_all(&dir).expect("removed");
        assert_eq!(loaded, Some(preferences));
    }

    #[test]
    fn missing_fields_take_defaults() {
        let dir = temp_dir("partial");
        let path = dir.join(PREFERENCES_FILE);
        fs::create_dir_all(&dir).expect("created");
        fs::write(&path, r#"{"onboarded":true}"#).expect("written");
        let loaded = load(&path);
        fs::remove_dir_all(&dir).expect("removed");
        assert_eq!(
            loaded,
            Some(Preferences {
                onboarded: true,
                ..Preferences::default()
            })
        );
    }

    #[test]
    fn a_broken_file_is_ignored() {
        let dir = temp_dir("broken");
        let path = dir.join(PREFERENCES_FILE);
        fs::create_dir_all(&dir).expect("created");
        fs::write(&path, "{not json").expect("written");
        let loaded = load(&path);
        fs::remove_dir_all(&dir).expect("removed");
        assert_eq!(loaded, None);
    }

    #[test]
    fn a_missing_file_is_ignored() {
        assert_eq!(load(&temp_dir("missing").join(PREFERENCES_FILE)), None);
    }
}
