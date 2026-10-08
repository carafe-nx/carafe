//! Runtime archive: one file instead of ~1700, each file compressed separately and checked by SHA-256 on unpacking.
//!
//! Layout: 8 bytes of [`MAGIC`], the index length (`u64` LE), the index in JSON, then the file data
//! (raw deflate) back to back. Offsets in the index are from the start of the data.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use carafe_core::ports::AdapterError;
use carafe_core::runtime_files::{Damage, DamagedFile, describe};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::io_error;

/// The first bytes of a runtime archive.
pub const MAGIC: [u8; 8] = *b"CRFRUN01";

/// The runtime folders that go into the archive.
pub const ARCHIVED_DIRS: [&str; 4] = ["control", "exefs", "logo", "romfs"];

const MAX_INDEX_BYTES: u64 = 16 << 20;
const BUFFER_SIZE: usize = 1 << 20;

/// A file in the archive index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Path from the runtime root, separated by `/`.
    pub path: String,
    /// Size after unpacking.
    pub size: u64,
    /// Size in the archive.
    pub packed: u64,
    /// Offset in the archive from the start of the data.
    pub offset: u64,
    /// SHA-256 of the unpacked file in hexadecimal.
    pub sha256: String,
}

#[derive(Serialize, Deserialize)]
struct Index {
    files: Vec<ArchiveEntry>,
}

/// An open runtime archive.
pub struct RuntimeArchive {
    path: PathBuf,
    data_start: u64,
    entries: Vec<ArchiveEntry>,
}

impl RuntimeArchive {
    /// Reads the archive index.
    ///
    /// # Errors
    ///
    /// [`AdapterError::RuntimeDamaged`] if the file does not exist, cannot be read or is not a runtime archive.
    pub fn open(path: &Path) -> Result<Self, AdapterError> {
        let damaged =
            |reason: String| AdapterError::RuntimeDamaged(format!("{}: {reason}", path.display()));
        let mut file = File::open(path).map_err(|error| damaged(error.to_string()))?;
        let mut head = [0; 16];
        file.read_exact(&mut head)
            .map_err(|error| damaged(error.to_string()))?;
        if head[..8] != MAGIC {
            return Err(damaged("not a Carafe runtime archive".to_owned()));
        }
        let mut length = [0; 8];
        length.copy_from_slice(&head[8..]);
        let index_len = u64::from_le_bytes(length);
        if index_len > MAX_INDEX_BYTES {
            return Err(damaged(format!("index of {index_len} bytes")));
        }
        let mut index = vec![0; usize::try_from(index_len).unwrap_or(0)];
        file.read_exact(&mut index)
            .map_err(|error| damaged(error.to_string()))?;
        let index: Index =
            serde_json::from_slice(&index).map_err(|error| damaged(error.to_string()))?;
        Ok(Self {
            path: path.to_owned(),
            data_start: 16 + index_len,
            entries: index.files,
        })
    }

    /// Returns whether the archive has the file `path`.
    pub fn contains(&self, path: &str) -> bool {
        self.entries.iter().any(|entry| entry.path == path)
    }

    /// Returns the number of files under `prefix/`.
    pub fn file_count(&self, prefix: &str) -> usize {
        self.under(prefix).count()
    }

    /// Returns the size of all archive files after unpacking, in bytes.
    pub fn unpacked_size(&self) -> u64 {
        self.entries.iter().map(|entry| entry.size).sum()
    }

    /// Returns the paths of the files under `prefix/` relative to `prefix`, separated by `/`.
    pub fn paths<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> {
        self.under(prefix)
            .map(move |entry| &entry.path[prefix.len() + 1..])
    }

    /// Unpacks and checks one file.
    ///
    /// # Errors
    ///
    /// [`AdapterError::RuntimeDamaged`] if the archive has no such file, it cannot be unpacked or its SHA-256
    /// does not match the index.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, AdapterError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| {
                AdapterError::RuntimeDamaged(format!("{}: no {path}", self.path.display()))
            })?;
        let mut file = self.open_data()?;
        let mut bytes = Vec::with_capacity(usize::try_from(entry.size).unwrap_or(0));
        let digest = self.unpack(&mut file, entry, &mut bytes, &mut vec![0; BUFFER_SIZE]);
        match digest {
            Ok(sha256) if sha256 == entry.sha256 && bytes.len() as u64 == entry.size => Ok(bytes),
            _ => Err(AdapterError::RuntimeDamaged(describe(&[DamagedFile {
                path: path.to_owned(),
                damage: Damage::Changed,
            }]))),
        }
    }

    /// Unpacks the files from `prefix/` into the folder `dest`, keeping the nesting, and checks each against SHA-256.
    ///
    /// `on_percent` receives the progress 0…100 by unpacked bytes, each value at most once.
    ///
    /// # Returns
    ///
    /// The number of unpacked files.
    ///
    /// # Errors
    ///
    /// [`AdapterError::RuntimeDamaged`] with the list of files that failed to unpack or did not match
    /// the index; [`AdapterError::Io`] or [`AdapterError::Blocked`] if `dest` cannot be written to.
    pub fn extract(
        &self,
        prefix: &str,
        dest: &Path,
        on_percent: &mut dyn FnMut(u8),
    ) -> Result<usize, AdapterError> {
        let entries: Vec<&ArchiveEntry> = self.under(prefix).collect();
        let total: u64 = entries.iter().map(|entry| entry.size).sum();
        let mut file = self.open_data()?;
        let mut buffer = vec![0; BUFFER_SIZE];
        let mut damaged = Vec::new();
        let mut done = 0u64;
        let mut reported = None;
        for entry in &entries {
            let relative = &entry.path[prefix.len() + 1..];
            let target = relative
                .split('/')
                .fold(dest.to_path_buf(), |path, part| path.join(part));
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
            }
            let out = File::create(&target).map_err(|error| io_error(&target, &error))?;
            let mut out = BufWriter::with_capacity(BUFFER_SIZE, out);
            let digest = self.unpack(&mut file, entry, &mut out, &mut buffer);
            out.flush().map_err(|error| io_error(&target, &error))?;
            match digest {
                Ok(sha256) if sha256 == entry.sha256 => {}
                Ok(_) => damaged.push(DamagedFile {
                    path: entry.path.clone(),
                    damage: Damage::Changed,
                }),
                Err(Unpack::Write(error)) => return Err(io_error(&target, &error)),
                Err(Unpack::Read) => damaged.push(DamagedFile {
                    path: entry.path.clone(),
                    damage: Damage::Unreadable,
                }),
            }
            done += entry.size;
            let percent = u8::try_from(done.min(total) * 100 / total.max(1)).unwrap_or(100);
            if reported != Some(percent) {
                reported = Some(percent);
                on_percent(percent);
            }
        }
        if !damaged.is_empty() {
            return Err(AdapterError::RuntimeDamaged(describe(&damaged)));
        }
        Ok(entries.len())
    }

    fn under<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a ArchiveEntry> {
        self.entries.iter().filter(move |entry| {
            entry
                .path
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    fn open_data(&self) -> Result<File, AdapterError> {
        File::open(&self.path).map_err(|error| {
            AdapterError::RuntimeDamaged(format!("{}: {error}", self.path.display()))
        })
    }

    fn unpack(
        &self,
        file: &mut File,
        entry: &ArchiveEntry,
        out: &mut dyn Write,
        buffer: &mut [u8],
    ) -> Result<String, Unpack> {
        file.seek(SeekFrom::Start(self.data_start + entry.offset))
            .map_err(|_| Unpack::Read)?;
        let packed = BufReader::new(Read::by_ref(file).take(entry.packed));
        let mut decoder = DeflateDecoder::new(packed);
        let mut hasher = Sha256::new();
        loop {
            let read = match decoder.read(buffer) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(Unpack::Read),
            };
            hasher.update(&buffer[..read]);
            out.write_all(&buffer[..read]).map_err(Unpack::Write)?;
        }
        Ok(hex(&hasher.finalize()))
    }
}

enum Unpack {
    Read,
    Write(io::Error),
}

/// Builds the runtime archive from the folder `runtime` (the `out/runtime-test` layout): the folders
/// [`ARCHIVED_DIRS`] are taken, paths are sorted, each file is compressed separately.
///
/// `on_file` receives the path of each added file.
///
/// # Returns
///
/// The number of files in the archive.
///
/// # Errors
///
/// [`AdapterError::NotFound`] if any of the folders is missing; [`AdapterError::Io`] on a read
/// or write error.
pub fn write_archive(
    runtime: &Path,
    out: &Path,
    on_file: &mut dyn FnMut(&str),
) -> Result<usize, AdapterError> {
    let mut files = Vec::new();
    for dir in ARCHIVED_DIRS {
        collect(runtime, Path::new(dir), &mut files)?;
    }
    files.sort();
    let data_path = out.with_extension("data.part");
    let mut entries = Vec::with_capacity(files.len());
    let mut offset = 0u64;
    {
        let data = File::create(&data_path).map_err(|error| io_error(&data_path, &error))?;
        let mut data = BufWriter::with_capacity(BUFFER_SIZE, data);
        let mut buffer = vec![0; BUFFER_SIZE];
        for path in &files {
            let source = path
                .split('/')
                .fold(runtime.to_path_buf(), |full, part| full.join(part));
            let mut input = File::open(&source).map_err(|error| io_error(&source, &error))?;
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
            let mut hasher = Sha256::new();
            let mut size = 0u64;
            loop {
                let read = input
                    .read(&mut buffer)
                    .map_err(|error| io_error(&source, &error))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                encoder
                    .write_all(&buffer[..read])
                    .map_err(|error| io_error(&source, &error))?;
                size += read as u64;
            }
            let packed = encoder
                .finish()
                .map_err(|error| io_error(&source, &error))?;
            data.write_all(&packed)
                .map_err(|error| io_error(&data_path, &error))?;
            entries.push(ArchiveEntry {
                path: path.clone(),
                size,
                packed: packed.len() as u64,
                offset,
                sha256: hex(&hasher.finalize()),
            });
            offset += packed.len() as u64;
            on_file(path);
        }
        data.flush().map_err(|error| io_error(&data_path, &error))?;
    }
    let index = serde_json::to_vec(&Index { files: entries })
        .map_err(|error| AdapterError::Io(error.to_string()))?;
    let result = (|| -> io::Result<()> {
        let mut archive = BufWriter::new(File::create(out)?);
        archive.write_all(&MAGIC)?;
        archive.write_all(&(index.len() as u64).to_le_bytes())?;
        archive.write_all(&index)?;
        io::copy(&mut File::open(&data_path)?, &mut archive)?;
        archive.flush()
    })();
    let _ = fs::remove_file(&data_path);
    result.map_err(|error| io_error(out, &error))?;
    Ok(files.len())
}

fn collect(root: &Path, relative: &Path, files: &mut Vec<String>) -> Result<(), AdapterError> {
    let dir = root.join(relative);
    for entry in fs::read_dir(&dir).map_err(|error| io_error(&dir, &error))? {
        let entry = entry.map_err(|error| io_error(&dir, &error))?;
        let child = relative.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|error| io_error(&entry.path(), &error))?;
        if kind.is_dir() {
            collect(root, &child, files)?;
        } else {
            let parts: Vec<String> = child
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.push(parts.join("/"));
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("carafe-archive-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        let _ = fs::remove_file(&path);
        path
    }

    fn runtime(name: &str) -> PathBuf {
        let root = temp(name);
        let files: [(&str, &[u8]); 5] = [
            ("exefs/main", b"loader"),
            ("exefs/main.npdm", b"npdm"),
            ("control/icon_AmericanEnglish.dat", b"icon"),
            ("romfs/switch/wine/a.dll", &[7; 5000]),
            ("romfs/switch/wine/sub/b.dll", b"bbb"),
        ];
        for (path, bytes) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fs::create_dir_all(root.join("logo")).unwrap();
        fs::create_dir_all(root.join("symbols")).unwrap();
        fs::write(root.join("symbols/runtime.elf"), b"not archived").unwrap();
        root
    }

    #[test]
    fn archive_round_trips_and_skips_symbols() {
        let source = runtime("round-trip");
        let archive_path = temp("round-trip.bin");
        let count = write_archive(&source, &archive_path, &mut |_| {}).unwrap();
        assert_eq!(count, 5);
        let archive = RuntimeArchive::open(&archive_path).unwrap();
        assert!(archive.contains("exefs/main"));
        assert!(!archive.contains("symbols/runtime.elf"));
        assert_eq!(archive.read("exefs/main.npdm").unwrap(), b"npdm");
        assert_eq!(archive.file_count("romfs"), 2);
        let dest = temp("round-trip-out");
        let mut last = None;
        let extracted = archive
            .extract("romfs", &dest, &mut |percent| last = Some(percent))
            .unwrap();
        assert_eq!(extracted, 2);
        assert_eq!(last, Some(100));
        assert_eq!(fs::read(dest.join("switch/wine/a.dll")).unwrap(), [7; 5000]);
        assert_eq!(
            fs::read(dest.join("switch/wine/sub/b.dll")).unwrap(),
            b"bbb"
        );
        fs::remove_dir_all(&source).unwrap();
        fs::remove_dir_all(&dest).unwrap();
        fs::remove_file(&archive_path).unwrap();
    }

    #[test]
    fn corrupted_data_is_reported_as_damaged_runtime() {
        let source = runtime("corrupt");
        let archive_path = temp("corrupt.bin");
        write_archive(&source, &archive_path, &mut |_| {}).unwrap();
        let mut bytes = fs::read(&archive_path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        fs::write(&archive_path, &bytes).unwrap();
        let archive = RuntimeArchive::open(&archive_path).unwrap();
        let dest = temp("corrupt-out");
        let result = archive.extract("romfs", &dest, &mut |_| {});
        fs::remove_dir_all(&source).unwrap();
        let _ = fs::remove_dir_all(&dest);
        fs::remove_file(&archive_path).unwrap();
        assert!(matches!(result, Err(AdapterError::RuntimeDamaged(_))));
    }

    #[test]
    fn missing_or_foreign_file_is_a_damaged_runtime() {
        assert!(matches!(
            RuntimeArchive::open(Path::new("Z:/carafe/missing/runtime.bin")),
            Err(AdapterError::RuntimeDamaged(_))
        ));
        let foreign = temp("foreign.bin");
        fs::write(&foreign, b"PK\x03\x04 a zip, not ours....").unwrap();
        let result = RuntimeArchive::open(&foreign);
        fs::remove_file(&foreign).unwrap();
        assert!(matches!(result, Err(AdapterError::RuntimeDamaged(_))));
    }
}
