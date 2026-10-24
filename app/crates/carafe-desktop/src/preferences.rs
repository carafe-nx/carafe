//! Settings of the application itself and their file.

use std::fs;
use std::io;
use std::path::Path;

use carafe_core::settings::AutorunSettings;
use carafe_core::update::UpdateMode;
use carafe_core::version::Version;
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
    /// Simplified Chinese.
    #[serde(rename = "zh-Hans")]
    #[ts(rename = "zh-Hans")]
    ZhHans,
    /// Japanese.
    Ja,
    /// Korean.
    Ko,
    /// Traditional Chinese.
    #[serde(rename = "zh-Hant")]
    #[ts(rename = "zh-Hant")]
    ZhHant,
    /// French.
    Fr,
    /// Canadian French.
    #[serde(rename = "fr-CA")]
    #[ts(rename = "fr-CA")]
    FrCa,
    /// German.
    De,
    /// Italian.
    It,
    /// Spanish as spoken in Spain.
    Es,
    /// Latin American Spanish.
    #[serde(rename = "es-419")]
    #[ts(rename = "es-419")]
    Es419,
    /// Dutch.
    Nl,
    /// Portuguese as spoken in Portugal.
    Pt,
    /// Brazilian Portuguese.
    #[serde(rename = "pt-BR")]
    #[ts(rename = "pt-BR")]
    PtBr,
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
    /// How Carafe looks for its new versions.
    pub updates: UpdateMode,
    /// The version the player chose not to be told about.
    #[ts(type = "string | null")]
    pub skipped_update: Option<Version>,
    /// The runtime version for which the player chose not to be offered a rebuild.
    pub rebuild_dismissed: Option<String>,
}

/// Returns the settings from the file.
///
/// Returns `None` if the file is missing, unreadable or cannot be parsed: the application then starts
/// with the default settings. Fields missing from the file are taken from [`Preferences::default`]; a
/// language this version does not know reads as [`Language::System`].
pub fn load(path: &Path) -> Option<Preferences> {
    let text = fs::read_to_string(path).ok()?;
    let mut value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if let Some(language) = value.get_mut("language")
        && serde_json::from_value::<Language>(language.clone()).is_err()
    {
        *language = serde_json::Value::from("system");
    }
    serde_json::from_value(value).ok()
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

    #[test]
    fn every_language_round_trips_under_its_code() {
        let languages = [
            (Language::System, "system"),
            (Language::Ru, "ru"),
            (Language::En, "en"),
            (Language::ZhHans, "zh-Hans"),
            (Language::Ja, "ja"),
            (Language::Ko, "ko"),
            (Language::ZhHant, "zh-Hant"),
            (Language::Fr, "fr"),
            (Language::FrCa, "fr-CA"),
            (Language::De, "de"),
            (Language::It, "it"),
            (Language::Es, "es"),
            (Language::Es419, "es-419"),
            (Language::Nl, "nl"),
            (Language::Pt, "pt"),
            (Language::PtBr, "pt-BR"),
        ];
        for (language, code) in languages {
            let preferences = Preferences {
                language,
                ..Preferences::default()
            };
            let json = serde_json::to_string(&preferences).expect("serialized");
            assert!(json.contains(&format!(r#""language":"{code}""#)), "{json}");
            assert_eq!(
                serde_json::from_str::<Preferences>(&json).expect("parsed"),
                preferences
            );
        }
    }

    #[test]
    fn an_unknown_language_reads_as_the_system_one() {
        let dir = temp_dir("unknown-language");
        let path = dir.join(PREFERENCES_FILE);
        fs::create_dir_all(&dir).expect("created");
        let mut loaded = Vec::new();
        for code in [r#""pl""#, r#""zh_Hant""#, r#""""#, "7", "null"] {
            let json = format!(r#"{{"onboarded":true,"language":{code},"theme":"dark"}}"#);
            fs::write(&path, json).expect("written");
            loaded.push(load(&path));
        }
        fs::remove_dir_all(&dir).expect("removed");
        for preferences in loaded {
            let preferences = preferences.expect("loaded");
            assert!(preferences.onboarded);
            assert_eq!(preferences.language, Language::System);
            assert_eq!(preferences.theme, Theme::Dark);
        }
    }

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
            keys_path: Some("C:/Switch/prod.keys".to_owned()),
            library_dir: Some("C:/Library".to_owned()),
            defaults: AutorunSettings::default(),
            steam_grid_db_key: Some("0123abcd".to_owned()),
            updates: UpdateMode::NotifyOnly,
            skipped_update: Version::parse("0.3.0").ok(),
            rebuild_dismissed: Some("0.2.0".to_owned()),
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
