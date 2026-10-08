//! The library's service folder `.carafe`: temporary build folders, their locks and cleanup of leftovers.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use carafe_core::ports::AdapterError;
use fs4::{FileExt, TryLockError};

use crate::io_error;

/// The service folder inside the library folder.
pub const SERVICE_DIR: &str = ".carafe";

/// The folder of temporary builds inside the service folder.
pub const BUILD_DIR: &str = "build";

const LOCK_FILE: &str = ".lock";
const PARTIAL_SUFFIX: &str = ".nsp.part";

/// Returns the temporary build folder of the game `name` in the library `library`.
pub fn build_dir(library: &Path, name: &str) -> PathBuf {
    library.join(SERVICE_DIR).join(BUILD_DIR).join(name)
}

/// The lock of a temporary build: while it is alive, [`is_busy`] returns `true` for its folder.
pub struct BuildLock {
    _file: File,
}

impl BuildLock {
    /// Creates a lock file in the folder `dir` and locks it.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Io`] if the file cannot be created; [`AdapterError::Tool`] if another process has already
    /// locked it.
    pub fn acquire(dir: &Path) -> Result<Self, AdapterError> {
        let path = dir.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|error| io_error(&path, &error))?;
        match FileExt::try_lock(&file) {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(already_running(dir)),
            Err(TryLockError::Error(error)) => Err(io_error(&path, &error)),
        }
    }
}

/// Returns the "another build of this game is running" error for the build folder `dir`.
pub fn already_running(dir: &Path) -> AdapterError {
    AdapterError::Tool(format!(
        "{}: another build of this game is running",
        dir.display()
    ))
}

/// Returns whether a build is currently running in the folder `dir`.
///
/// A folder without a lock is not busy; a lock that cannot be opened or checked counts as busy.
pub fn is_busy(dir: &Path) -> bool {
    let path = dir.join(LOCK_FILE);
    let file = match OpenOptions::new().read(true).write(true).open(&path) {
        Ok(file) => file,
        Err(error) => return error.kind() != ErrorKind::NotFound,
    };
    !matches!(FileExt::try_lock(&file), Ok(()))
}

/// Removes leftovers of interrupted builds: folders in `.carafe/build` and unfinished `<name>.nsp.part` in the library.
///
/// Folders where a build is running, and their `.nsp.part`, are left alone. The service folder, if present, becomes
/// hidden.
///
/// # Returns
///
/// The number of removed folders and files; zero if there is nothing to clean up or the library folder does not exist.
///
/// # Errors
///
/// [`AdapterError::Io`] if the builds folder or the library folder cannot be read.
pub fn clean(library: &Path) -> Result<usize, AdapterError> {
    let service = library.join(SERVICE_DIR);
    if service.is_dir() {
        hide(&service);
    }
    let builds = service.join(BUILD_DIR);
    let mut removed = 0;
    for entry in entries(&builds)? {
        let path = entry.path();
        if path.is_dir() && !is_busy(&path) && fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    for entry in entries(library)? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(title) = name.strip_suffix(PARTIAL_SUFFIX) else {
            continue;
        };
        let path = entry.path();
        if path.is_file() && !is_busy(&builds.join(title)) && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn entries(dir: &Path) -> Result<Vec<fs::DirEntry>, AdapterError> {
    match fs::read_dir(dir) {
        Ok(entries) => entries
            .collect::<Result<_, _>>()
            .map_err(|error| io_error(dir, &error)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(io_error(dir, &error)),
    }
}

/// Makes the folder `path` hidden in Explorer if it is not hidden yet; a failure does not count as an error.
#[cfg(target_os = "windows")]
pub fn hide(path: &Path) {
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let hidden = fs::metadata(path)
        .is_ok_and(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0);
    if !hidden {
        let _ = Command::new("attrib")
            .arg("+h")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
}

/// Does nothing: outside Windows a folder whose name starts with a dot is hidden anyway.
#[cfg(not(target_os = "windows"))]
pub fn hide(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn library(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("carafe-service-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_locked_build_is_busy_until_the_lock_is_dropped() {
        let library = library("lock");
        let dir = build_dir(&library, "0556710511f9c000");
        fs::create_dir_all(&dir).unwrap();
        assert!(!is_busy(&dir));
        let lock = BuildLock::acquire(&dir).unwrap();
        assert!(is_busy(&dir));
        assert!(matches!(
            BuildLock::acquire(&dir),
            Err(AdapterError::Tool(_))
        ));
        drop(lock);
        assert!(!is_busy(&dir));
        fs::remove_dir_all(&library).unwrap();
    }

    #[test]
    fn clean_removes_leftovers_and_keeps_running_builds() {
        let library = library("clean");
        let stale = build_dir(&library, "0556710511f9c000");
        let running = build_dir(&library, "05bb530520ee2000");
        fs::create_dir_all(stale.join("in/romfs")).unwrap();
        fs::write(stale.join("in/romfs/file.txt"), b"x").unwrap();
        fs::create_dir_all(&running).unwrap();
        let lock = BuildLock::acquire(&running).unwrap();
        fs::write(library.join("0556710511f9c000.nsp.part"), b"x").unwrap();
        fs::write(library.join("05bb530520ee2000.nsp.part"), b"x").unwrap();
        fs::write(library.join("05d6bd5f14aa0000.nsp"), b"x").unwrap();

        assert_eq!(clean(&library).unwrap(), 2);

        assert!(!stale.exists());
        assert!(running.is_dir());
        assert!(!library.join("0556710511f9c000.nsp.part").exists());
        assert!(library.join("05bb530520ee2000.nsp.part").exists());
        assert!(library.join("05d6bd5f14aa0000.nsp").exists());
        drop(lock);
        fs::remove_dir_all(&library).unwrap();
    }

    #[test]
    fn clean_of_a_missing_library_removes_nothing() {
        let missing =
            std::env::temp_dir().join(format!("carafe-service-{}-missing", std::process::id()));
        let _ = fs::remove_dir_all(&missing);
        assert_eq!(clean(&missing).unwrap(), 0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn the_service_folder_becomes_hidden() {
        use std::os::windows::fs::MetadataExt;

        let library = library("hide");
        let service = library.join(SERVICE_DIR);
        fs::create_dir_all(&service).unwrap();
        clean(&library).unwrap();
        let attributes = fs::metadata(&service).unwrap().file_attributes();
        fs::remove_dir_all(&library).unwrap();
        assert_ne!(attributes & 0x2, 0);
    }
}
