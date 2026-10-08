//! The library on disk: an adapter for the [`GameStore`] port.

use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom};
use std::path::Path;

use carafe_core::TitleId;
use carafe_core::icon::{IconJpeg, MAX_ICON_BYTES, data_url};
use carafe_core::library::newest_per_title;
use carafe_core::package::{ICON_FILE, RECORD_FILE};
use carafe_core::pfs0::{self, PREFIX_SIZE, Pfs0File};
use carafe_core::ports::{AdapterError, GameStore, StoredGame};
use carafe_core::record::BuildRecord;

/// A `carafe.json` larger than this is not read.
pub const MAX_RECORD_BYTES: u64 = 1 << 20;

/// Reads NSP files from the library folder; subfolders, including `.carafe`, are not scanned.
pub struct DiskGameStore;

impl GameStore for DiskGameStore {
    fn list(&self, library_dir: &str) -> Result<Vec<StoredGame>, AdapterError> {
        let dir = Path::new(library_dir);
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(dir, &error)),
        };
        let games = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let nsp = name.to_ascii_lowercase().ends_with(".nsp");
                nsp.then(|| read_game(&entry.path(), name)).flatten()
            })
            .collect();
        Ok(newest_per_title(games))
    }

    fn delete(&self, library_dir: &str, title_id: TitleId) -> Result<(), AdapterError> {
        let game = self
            .list(library_dir)?
            .into_iter()
            .find(|game| game.record.title_id == title_id)
            .ok_or_else(|| AdapterError::NotFound(title_id.to_string()))?;
        let path = Path::new(library_dir).join(&game.file_name);
        fs::remove_file(&path).map_err(|error| io_error(&path, &error))
    }
}

fn read_game(path: &Path, file_name: String) -> Option<StoredGame> {
    let mut file = File::open(path).ok()?;
    let size_bytes = file.metadata().ok()?.len();
    let prefix = read_at(&mut file, size_bytes, 0, PREFIX_SIZE as u64)?;
    let header_size = pfs0::header_size(&prefix).ok()?;
    let header = read_at(&mut file, size_bytes, 0, header_size as u64)?;
    let files = pfs0::files(&header).ok()?;
    let record_file = files.iter().find(|entry| entry.name == RECORD_FILE)?;
    if record_file.size > MAX_RECORD_BYTES {
        return None;
    }
    let json = read_entry(&mut file, size_bytes, record_file)?;
    let record: BuildRecord = serde_json::from_slice(&json).ok()?;
    record.check_format().ok()?;
    let icon = files
        .iter()
        .find(|entry| entry.name == ICON_FILE && entry.size <= MAX_ICON_BYTES as u64)
        .and_then(|entry| read_entry(&mut file, size_bytes, entry))
        .and_then(|bytes| IconJpeg::new(bytes).ok())
        .map(|icon| data_url("image/jpeg", icon.bytes()));
    Some(StoredGame {
        record,
        file_name,
        size_bytes,
        icon,
    })
}

fn read_entry(file: &mut File, size: u64, entry: &Pfs0File) -> Option<Vec<u8>> {
    read_at(file, size, entry.offset, entry.size)
}

fn read_at(file: &mut File, size: u64, offset: u64, len: u64) -> Option<Vec<u8>> {
    if offset.checked_add(len)? > size {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = vec![0; usize::try_from(len).ok()?];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn io_error(path: &Path, error: &io::Error) -> AdapterError {
    let message = format!("{}: {error}", path.display());
    if error.kind() == ErrorKind::NotFound {
        AdapterError::NotFound(message)
    } else {
        AdapterError::Io(message)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use carafe_core::metadata::Metadata;
    use carafe_core::pfs0::Pfs0Entry;
    use carafe_core::record::{Arch, FORMAT_VERSION, GameSource};
    use carafe_core::settings::AutorunSettings;

    use super::*;

    fn record(entropy: u64, build_number: u32) -> BuildRecord {
        BuildRecord {
            format_version: FORMAT_VERSION,
            title_id: TitleId::from_entropy(entropy),
            build_number,
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

    fn nsp(files: &[(&str, &[u8])]) -> Vec<u8> {
        let entries: Vec<Pfs0Entry> = files
            .iter()
            .map(|(name, bytes)| Pfs0Entry {
                name: (*name).to_owned(),
                size: bytes.len() as u64,
            })
            .collect();
        let mut out = pfs0::header(&entries).unwrap();
        for (_, bytes) in files {
            out.extend_from_slice(bytes);
        }
        out
    }

    fn carafe_nsp(record: &BuildRecord) -> Vec<u8> {
        let json = serde_json::to_vec(record).unwrap();
        nsp(&[("0123.nca", b"nca data"), (RECORD_FILE, &json)])
    }

    fn library(name: &str, files: &[(&str, Vec<u8>)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("carafe-library-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (file, bytes) in files {
            fs::write(dir.join(file), bytes).unwrap();
        }
        dir
    }

    #[test]
    fn carafe_nsps_are_listed_and_others_skipped() {
        let game = record(1, 3);
        let dir = library(
            "list",
            &[
                ("game.nsp", carafe_nsp(&game)),
                ("foreign.nsp", nsp(&[("a.nca", b"x")])),
                ("broken.nsp", b"PFS0 and nothing more".to_vec()),
                ("notes.txt", b"text".to_vec()),
                ("half.nsp.part", carafe_nsp(&record(2, 1))),
            ],
        );
        let games = DiskGameStore.list(dir.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].record, game);
        assert_eq!(games[0].file_name, "game.nsp");
        assert_eq!(games[0].size_bytes, carafe_nsp(&game).len() as u64);
        assert_eq!(games[0].icon, None);
    }

    #[test]
    fn newer_format_is_skipped() {
        let mut future = record(1, 1);
        future.format_version = FORMAT_VERSION + 1;
        let dir = library("future", &[("future.nsp", carafe_nsp(&future))]);
        let games = DiskGameStore.list(dir.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn missing_folder_is_an_empty_library() {
        let games = DiskGameStore.list("Z:/carafe/missing/library").unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn delete_removes_the_listed_nsp() {
        let old = record(1, 1);
        let new = record(1, 2);
        let dir = library(
            "delete",
            &[("old.nsp", carafe_nsp(&old)), ("new.nsp", carafe_nsp(&new))],
        );
        let path = dir.to_str().unwrap();
        DiskGameStore.delete(path, new.title_id).unwrap();
        let left = DiskGameStore.list(path).unwrap();
        let missing = DiskGameStore.delete(path, TitleId::from_entropy(9));
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].file_name, "old.nsp");
        assert!(matches!(missing, Err(AdapterError::NotFound(_))));
    }
}
