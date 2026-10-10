//! Library of finished NSPs.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::icon::data_url;
use crate::ports::StoredGame;
use crate::record::Arch;
use crate::title_id::TitleId;
use crate::version::runtime_outdated;

/// Library tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameSummary {
    /// Title ID.
    pub title_id: TitleId,
    /// Title.
    pub title: String,
    /// Publisher.
    pub publisher: String,
    /// Version for display.
    pub version: String,
    /// Bitness of the `.exe`.
    pub arch: Arch,
    /// NSP size in bytes.
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// Icon as a `data:` URL.
    pub icon: Option<String>,
    /// Runtime version the NSP was built with.
    pub runtime_version: String,
    /// The runtime in Carafe is newer than in the NSP.
    pub runtime_outdated: bool,
}

/// Returns the library tiles sorted by title, case-insensitively.
#[must_use]
pub fn summaries(games: &[StoredGame], current_runtime: &str) -> Vec<GameSummary> {
    let mut tiles: Vec<GameSummary> = games
        .iter()
        .map(|game| {
            let record = &game.record;
            GameSummary {
                title_id: record.title_id,
                title: record.metadata.title.clone(),
                publisher: record.metadata.publisher.clone(),
                version: record.display_version(),
                arch: record.source.arch,
                size_bytes: game.size_bytes,
                icon: game
                    .icon
                    .as_ref()
                    .map(|icon| data_url("image/jpeg", icon.bytes())),
                runtime_version: record.runtime_version.clone(),
                runtime_outdated: runtime_outdated(&record.runtime_version, current_runtime),
            }
        })
        .collect();
    tiles.sort_by_key(|tile| tile.title.to_lowercase());
    tiles
}

/// Keeps one NSP per Title ID: the one with the highest build number, on a tie, the one with the smaller
/// file name.
///
/// Several NSPs of one game appear when the user copies or renames files themselves.
#[must_use]
pub fn newest_per_title(mut games: Vec<StoredGame>) -> Vec<StoredGame> {
    games.sort_by(|a, b| {
        a.record
            .title_id
            .cmp(&b.record.title_id)
            .then(b.record.build_number.cmp(&a.record.build_number))
            .then(a.file_name.cmp(&b.file_name))
    });
    games.dedup_by_key(|game| game.record.title_id);
    games
}

/// Name of the library folder that Carafe appends to the chosen folder.
pub const LIBRARY_FOLDER_NAME: &str = "Carafe";

/// Returns the library folder for the folder the user picked.
///
/// If `picked` is already named [`LIBRARY_FOLDER_NAME`], ignoring case, returns it as is,
/// otherwise the [`LIBRARY_FOLDER_NAME`] folder nested in it.
#[must_use]
pub fn library_dir_for(picked: &Path) -> PathBuf {
    let named = picked
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(LIBRARY_FOLDER_NAME));
    if named {
        picked.to_path_buf()
    } else {
        picked.join(LIBRARY_FOLDER_NAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::Metadata;
    use crate::record::{BuildRecord, FORMAT_VERSION, GameSource};
    use crate::settings::AutorunSettings;

    fn game(title: &str, runtime: &str, entropy: u64) -> StoredGame {
        StoredGame {
            record: BuildRecord {
                format_version: FORMAT_VERSION,
                title_id: TitleId::from_entropy(entropy),
                build_number: 1,
                runtime_version: runtime.to_owned(),
                source: GameSource {
                    folder: String::new(),
                    executable: "game.exe".to_owned(),
                    arch: Arch::X86,
                    arguments: Vec::new(),
                },
                metadata: Metadata::new(title),
                settings: AutorunSettings::default(),
            },
            file_name: format!("{title}.nsp"),
            size_bytes: 1,
            icon: None,
        }
    }

    #[test]
    fn sorted_by_title_ignoring_case() {
        let tiles = summaries(
            &[game("heroes", "0.1.0", 1), game("Diablo", "0.1.0", 2)],
            "0.1.0",
        );
        let titles: Vec<_> = tiles.iter().map(|tile| tile.title.as_str()).collect();
        assert_eq!(titles, ["Diablo", "heroes"]);
    }

    #[test]
    fn one_nsp_per_title_the_newest_build() {
        let old = game("old", "0.1.0", 1);
        let mut copy = old.clone();
        copy.file_name = "copy of old.nsp".to_owned();
        let mut rebuilt = old.clone();
        rebuilt.record.build_number = 2;
        rebuilt.file_name = "rebuilt.nsp".to_owned();
        let other = game("other", "0.1.0", 2);
        let kept = newest_per_title(vec![old, copy, rebuilt, other]);
        let names: Vec<_> = kept.iter().map(|game| game.file_name.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"rebuilt.nsp"));
        assert!(names.contains(&"other.nsp"));
    }

    #[test]
    fn marks_outdated_runtime() {
        let tiles = summaries(&[game("OpenTTD", "0.1.0", 1)], "0.2.0");
        assert!(tiles[0].runtime_outdated);
    }

    #[test]
    fn a_newer_runtime_is_not_outdated() {
        let tiles = summaries(&[game("OpenTTD", "0.3.0", 1)], "0.2.0");
        assert!(!tiles[0].runtime_outdated);
    }

    #[test]
    fn library_dir_appends_carafe() {
        let picked = Path::new("games");
        assert_eq!(library_dir_for(picked), picked.join("Carafe"));
    }

    #[test]
    fn library_dir_keeps_folder_named_carafe() {
        let picked = Path::new("games").join("carafe");
        assert_eq!(library_dir_for(&picked), picked);
    }

    #[test]
    fn library_dir_appends_carafe_to_drive_root() {
        let picked = Path::new("/");
        assert_eq!(library_dir_for(picked), picked.join("Carafe"));
    }
}
