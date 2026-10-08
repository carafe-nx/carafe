//! Checking `prod.keys` from disk: an adapter for the [`KeysChecker`] port.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use carafe_core::keys::inspect_keys;
use carafe_core::ports::{AdapterError, KeysChecker, KeysReport};

/// Key files larger than this are not read: a real `prod.keys` takes a few kilobytes.
pub const MAX_KEYS_FILE_BYTES: u64 = 1024 * 1024;

/// Reads `prod.keys` from the path in the settings and checks it with the core rule.
pub struct FileKeysChecker;

impl KeysChecker for FileKeysChecker {
    /// Returns which of the required keys are present in the file.
    ///
    /// The file contents stay in the memory of this call and are not passed anywhere. Bytes that are not
    /// UTF-8 are replaced and do not affect the result.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] if the file does not exist or the path is empty; [`AdapterError::Io`] if the
    /// file cannot be read or is larger than [`MAX_KEYS_FILE_BYTES`].
    fn check(&self, path: &str) -> Result<KeysReport, AdapterError> {
        if path.trim().is_empty() {
            return Err(AdapterError::NotFound(path.to_owned()));
        }
        let path = Path::new(path);
        let size = fs::metadata(path)
            .map_err(|error| io_error(path, &error))?
            .len();
        if size > MAX_KEYS_FILE_BYTES {
            return Err(AdapterError::Io(format!(
                "{}: {size} bytes, not a keys file",
                path.display()
            )));
        }
        let bytes = fs::read(path).map_err(|error| io_error(path, &error))?;
        Ok(inspect_keys(&String::from_utf8_lossy(&bytes)))
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> AdapterError {
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

    fn temp_file(name: &str, contents: &[u8]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("carafe-keys-test-{}-{name}", std::process::id()));
        fs::write(&path, contents).expect("write the test file");
        path
    }

    #[test]
    fn reads_keys_from_a_file() {
        let path = temp_file(
            "full.keys",
            b"header_key = 00\nkey_area_key_application_00 = 11\n",
        );
        let report = FileKeysChecker
            .check(path.to_str().expect("utf-8 path"))
            .expect("readable");
        fs::remove_file(&path).expect("remove the test file");
        assert!(report.header_key && report.key_area_key);
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let error = FileKeysChecker
            .check("Z:/carafe/definitely-missing/prod.keys")
            .expect_err("missing");
        assert!(matches!(error, AdapterError::NotFound(_)));
    }

    #[test]
    fn an_empty_path_is_not_found() {
        assert!(matches!(
            FileKeysChecker.check("  "),
            Err(AdapterError::NotFound(_))
        ));
    }

    #[test]
    fn binary_files_have_no_keys() {
        let path = temp_file("binary.keys", &[0xff, 0xfe, 0x00, 0x01]);
        let report = FileKeysChecker
            .check(path.to_str().expect("utf-8 path"))
            .expect("readable");
        fs::remove_file(&path).expect("remove the test file");
        assert!(!report.header_key && !report.key_area_key);
    }
}
