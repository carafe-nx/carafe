//! Build record: the contents of `carafe.json` inside the NSP.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::metadata::Metadata;
use crate::settings::AutorunSettings;
use crate::title_id::TitleId;

/// Version of the `carafe.json` format that this Carafe build writes.
pub const FORMAT_VERSION: u32 = 1;

/// Bitness of the game executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Arch {
    /// 32-bit x86.
    X86,
    /// 64-bit x86-64.
    X64,
}

/// Where the game was built from on the PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameSource {
    /// Game folder.
    pub folder: String,
    /// Path to the `.exe` relative to the folder.
    pub executable: String,
    /// Bitness of the `.exe`.
    pub arch: Arch,
    /// Launch arguments.
    pub arguments: Vec<String>,
}

/// Everything the NSP was built from: the contents of `carafe.json` inside it.
///
/// A full snapshot: changing the app's general settings does not affect a built game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildRecord {
    /// File format version.
    pub format_version: u32,
    /// Title ID, permanent for the game.
    pub title_id: TitleId,
    /// Build number of this game, starting from 1.
    pub build_number: u32,
    /// Carafe runtime version the NSP was built with.
    pub runtime_version: String,
    /// Where the game was built from.
    pub source: GameSource,
    /// NSP metadata.
    pub metadata: Metadata,
    /// Autorun settings.
    pub settings: AutorunSettings,
}

/// Error reading `carafe.json`.
#[derive(Debug, Error)]
pub enum RecordError {
    /// The file was written by a newer version of Carafe.
    #[error(
        "carafe.json version {0} is newer than this version of Carafe understands ({FORMAT_VERSION})"
    )]
    NewerFormat(u32),
    /// The file cannot be parsed.
    #[error("carafe.json cannot be read: {0}")]
    Malformed(String),
}

impl BuildRecord {
    /// Checks that this version of Carafe understands the record format.
    ///
    /// # Errors
    ///
    /// [`RecordError::NewerFormat`] if the record was made by a newer version.
    pub fn check_format(&self) -> Result<(), RecordError> {
        if self.format_version > FORMAT_VERSION {
            return Err(RecordError::NewerFormat(self.format_version));
        }
        Ok(())
    }

    /// Returns the record of the next build of the same game: the same Title ID, the build number one higher,
    /// the runtime `runtime_version`.
    #[must_use]
    pub fn next_build(&self, runtime_version: &str) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            build_number: self.build_number + 1,
            runtime_version: runtime_version.to_owned(),
            ..self.clone()
        }
    }

    /// Returns the version for display on the Switch.
    #[must_use]
    pub fn display_version(&self) -> String {
        self.metadata.version_for(self.build_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> BuildRecord {
        BuildRecord {
            format_version: FORMAT_VERSION,
            title_id: TitleId::parse("055deb23507ec000").unwrap(),
            build_number: 2,
            runtime_version: "0.1.0".to_owned(),
            source: GameSource {
                folder: "D:\\Game\\openttd".to_owned(),
                executable: "openttd.exe".to_owned(),
                arch: Arch::X86,
                arguments: Vec::new(),
            },
            metadata: Metadata::new("OpenTTD"),
            settings: AutorunSettings::default(),
        }
    }

    #[test]
    fn next_build_keeps_title_id() {
        let next = record().next_build("0.2.0");
        assert_eq!(next.title_id, record().title_id);
        assert_eq!(next.build_number, 3);
        assert_eq!(next.runtime_version, "0.2.0");
    }

    #[test]
    fn refuses_newer_format() {
        let newer = BuildRecord {
            format_version: FORMAT_VERSION + 1,
            ..record()
        };
        assert!(matches!(
            newer.check_format(),
            Err(RecordError::NewerFormat(_))
        ));
    }

    #[test]
    fn survives_json_round_trip() {
        let json = serde_json::to_string(&record()).unwrap();
        assert!(json.contains("\"titleId\":\"055deb23507ec000\""));
        assert_eq!(
            serde_json::from_str::<BuildRecord>(&json).unwrap(),
            record()
        );
    }
}
