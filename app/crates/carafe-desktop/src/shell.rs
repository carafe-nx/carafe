//! The system file manager and browser.

use std::io;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
const FILE_MANAGER: &str = "explorer";
#[cfg(target_os = "macos")]
const FILE_MANAGER: &str = "open";
#[cfg(all(unix, not(target_os = "macos")))]
const FILE_MANAGER: &str = "xdg-open";

/// Opens a folder in the system file manager without waiting for it.
///
/// # Errors
///
/// [`io::Error`] if the file manager failed to start.
pub fn open_folder(path: &Path) -> io::Result<()> {
    Command::new(FILE_MANAGER).arg(path).spawn().map(|_| ())
}

/// Opens a web page in the system browser without waiting for it.
///
/// # Errors
///
/// [`io::Error`] if the browser failed to start.
pub fn open_url(url: &str) -> io::Result<()> {
    Command::new(FILE_MANAGER).arg(url).spawn().map(|_| ())
}
