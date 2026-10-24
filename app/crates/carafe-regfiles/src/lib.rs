//! Windows registry files on a local drive: an adapter for the [`RegistryFiles`] port.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

use carafe_core::ports::{AdapterError, RegistryFiles};

/// Registry files larger than this are not read: a game's settings take far less.
pub const MAX_REG_BYTES: u64 = 16 << 20;

/// Reads and writes `.reg` files on a local drive.
pub struct DiskRegistryFiles;

impl RegistryFiles for DiskRegistryFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, AdapterError> {
        let path = Path::new(path);
        let size = fs::metadata(path)
            .map_err(|error| io_error(path, &error))?
            .len();
        if size > MAX_REG_BYTES {
            return Err(AdapterError::Unsupported(format!(
                "{}: {size} bytes, more than {MAX_REG_BYTES}",
                path.display()
            )));
        }
        fs::read(path).map_err(|error| io_error(path, &error))
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), AdapterError> {
        let path = Path::new(path);
        fs::write(path, bytes).map_err(|error| io_error(path, &error))
    }
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

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("carafe-regfiles-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("game.reg")
    }

    #[test]
    fn a_written_file_reads_back() {
        let path = scratch("roundtrip");
        let path_text = path.to_string_lossy();
        DiskRegistryFiles
            .write(&path_text, &[0xFF, 0xFE, b'R', 0])
            .unwrap();
        assert_eq!(
            DiskRegistryFiles.read(&path_text).unwrap(),
            vec![0xFF, 0xFE, b'R', 0]
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let path = scratch("missing").with_file_name("absent.reg");
        assert!(matches!(
            DiskRegistryFiles.read(&path.to_string_lossy()),
            Err(AdapterError::NotFound(_))
        ));
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
