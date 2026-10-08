//! Analysis of a game folder on disk: an adapter for the [`FolderInspector`] port.

use std::fs::{self, File};
use std::io::{self, ErrorKind, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use carafe_core::pe::{PeError, read_info};
use carafe_core::ports::{AdapterError, ExecutableInfo, FolderInspector, FolderReport};

/// Folders with more files and subfolders than this are not analyzed: that is more likely a whole drive than a game.
pub const MAX_ENTRIES: usize = 200_000;

const STEAM_API: [&str; 2] = ["steam_api.dll", "steam_api64.dll"];

/// Walks a game folder on a local drive.
///
/// Symbolic links and junction points are not followed; subfolders that cannot be read are skipped.
/// Only `.exe` files for x86 and x86-64 are listed: the runtime will not start DOS programs or ARM `.exe` files.
pub struct DiskFolderInspector;

impl FolderInspector for DiskFolderInspector {
    fn inspect(&self, folder: &str) -> Result<FolderReport, AdapterError> {
        let root = Path::new(folder);
        let metadata = fs::metadata(root).map_err(|error| io_error(root, &error))?;
        if !metadata.is_dir() {
            return Err(AdapterError::NotFound(format!(
                "{}: not a folder",
                root.display()
            )));
        }
        let mut executables = Vec::new();
        let mut has_steam_api = false;
        let mut seen = 0;
        let mut pending = vec![PathBuf::new()];
        while let Some(relative) = pending.pop() {
            let directory = root.join(&relative);
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if relative.as_os_str().is_empty() => {
                    return Err(io_error(root, &error));
                }
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                seen += 1;
                if seen > MAX_ENTRIES {
                    return Err(AdapterError::Unsupported(format!(
                        "{}: more than {MAX_ENTRIES} files and folders",
                        root.display()
                    )));
                }
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let name = entry.file_name();
                let path = relative.join(&name);
                if kind.is_dir() {
                    pending.push(path);
                    continue;
                }
                if !kind.is_file() {
                    continue;
                }
                let lower = name.to_string_lossy().to_lowercase();
                if STEAM_API.contains(&lower.as_str()) {
                    has_steam_api = true;
                }
                if lower.ends_with(".exe") {
                    executables.extend(executable(&root.join(&path), &path));
                }
            }
        }
        executables.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(FolderReport {
            folder: folder.to_owned(),
            executables,
            has_steam_api,
        })
    }
}

fn executable(path: &Path, relative: &Path) -> Option<ExecutableInfo> {
    let mut file = File::open(path).ok()?;
    let size_bytes = file.metadata().ok()?.len();
    let mut read = |offset: u64, len: usize| read_at(&mut file, size_bytes, offset, len);
    let info = match read_info(&mut read) {
        Ok(info) => info,
        Err(PeError::NotPe | PeError::BadResources) => return None,
    };
    Some(ExecutableInfo {
        path: windows_path(relative),
        arch: info.arch?,
        size_bytes,
        product_name: info.product_name,
        company_name: info.company_name,
    })
}

fn windows_path(relative: &Path) -> String {
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("\\")
}

fn read_at(file: &mut File, size: u64, offset: u64, len: usize) -> Option<Vec<u8>> {
    let end = offset.checked_add(len as u64)?;
    if end > size {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = vec![0; len];
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
    use carafe_core::record::Arch;

    use super::*;

    fn pe(machine: u16) -> Vec<u8> {
        let mut file = vec![0; 0x180];
        file[0..2].copy_from_slice(b"MZ");
        file[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        file[0x80..0x84].copy_from_slice(b"PE\0\0");
        file[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        file[0x94..0x96].copy_from_slice(&0xE0u16.to_le_bytes());
        file[0x98..0x9A].copy_from_slice(&0x10Bu16.to_le_bytes());
        file
    }

    fn game_folder(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("carafe-folder-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (path, bytes) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        root
    }

    fn inspect(root: &Path) -> FolderReport {
        let report = DiskFolderInspector.inspect(root.to_str().unwrap());
        fs::remove_dir_all(root).unwrap();
        report.unwrap()
    }

    #[test]
    fn executables_are_found_in_subfolders_with_their_arch() {
        let root = game_folder(
            "nested",
            &[
                ("Game.EXE", &pe(0x14C)),
                ("bin/x64/tool.exe", &pe(0x8664)),
                ("arm/native.exe", &pe(0xAA64)),
                ("dos/old.exe", b"MZ old dos program"),
                ("readme.txt", b"text"),
            ],
        );
        let report = inspect(&root);
        let found: Vec<(&str, Arch)> = report
            .executables
            .iter()
            .map(|info| (info.path.as_str(), info.arch))
            .collect();
        assert_eq!(
            found,
            vec![("Game.EXE", Arch::X86), ("bin\\x64\\tool.exe", Arch::X64)]
        );
        assert_eq!(report.executables[0].size_bytes, 0x180);
        assert!(!report.has_steam_api);
    }

    #[test]
    fn steam_api_is_noticed_anywhere() {
        let root = game_folder(
            "steam",
            &[("game.exe", &pe(0x14C)), ("Bin/Steam_Api64.dll", b"dll")],
        );
        assert!(inspect(&root).has_steam_api);
    }

    #[test]
    fn missing_folder_is_not_found() {
        assert!(matches!(
            DiskFolderInspector.inspect("Z:/carafe/missing/game"),
            Err(AdapterError::NotFound(_))
        ));
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let root = game_folder("file", &[("game.exe", &pe(0x14C))]);
        let result = DiskFolderInspector.inspect(root.join("game.exe").to_str().unwrap());
        fs::remove_dir_all(&root).unwrap();
        assert!(matches!(result, Err(AdapterError::NotFound(_))));
    }
}
