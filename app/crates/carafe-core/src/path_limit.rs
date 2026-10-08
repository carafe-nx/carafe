//! Length limit of the paths hacBrewPack reads when packing RomFS.

use thiserror::Error;

/// A RomFS path that hacBrewPack will not read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{path}: {length} UTF-8 bytes, limit {limit}")]
pub struct PathTooLong {
    /// Full path to the file or folder, as hacBrewPack joins it.
    pub path: String,
    /// Path length in UTF-8 bytes.
    pub length: usize,
    /// Limit in UTF-8 bytes.
    pub limit: usize,
}

impl PathTooLong {
    /// Returns by how many bytes the path exceeds the limit.
    #[must_use]
    pub fn excess(&self) -> usize {
        self.length - self.limit
    }
}

/// Checks that hacBrewPack will read every RomFS path.
///
/// The full path is `root`, a separator and the path inside RomFS; the length is counted in UTF-8 bytes.
///
/// # Arguments
///
/// * `root` — the RomFS folder in the form it is passed to hacBrewPack.
/// * `separator` — the system path separator; it replaces `/` in the path reported in the error.
/// * `entries` — RomFS files and folders, paths from its root separated by `/`.
/// * `limit` — the longest path in UTF-8 bytes that hacBrewPack still reads.
///
/// # Errors
///
/// [`PathTooLong`] with the longest path if it is longer than `limit` bytes.
pub fn check<'a>(
    root: &str,
    separator: char,
    entries: impl IntoIterator<Item = &'a str>,
    limit: usize,
) -> Result<(), PathTooLong> {
    let length = |entry: &str| root.len() + 1 + entry.len();
    let Some(longest) = entries.into_iter().max_by_key(|entry| length(entry)) else {
        return Ok(());
    };
    if length(longest) <= limit {
        return Ok(());
    }
    let relative = longest.replace('/', separator.encode_utf8(&mut [0; 4]));
    Err(PathTooLong {
        path: format!("{root}{separator}{relative}"),
        length: length(longest),
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r"D:\Carafe\.carafe\build\0556710511f9c000\in\romfs";

    #[test]
    fn a_path_of_exactly_the_limit_fits() {
        let entry = "a".repeat(259 - ROOT.len() - 1);
        assert_eq!(check(ROOT, '\\', [entry.as_str()], 259), Ok(()));
    }

    #[test]
    fn one_byte_over_the_limit_is_reported() {
        let entry = format!("switch/{}", "a".repeat(259 - ROOT.len() - 7));
        let error = check(ROOT, '\\', [entry.as_str()], 259).unwrap_err();
        assert_eq!(error.length, 260);
        assert_eq!(error.excess(), 1);
        assert_eq!(
            error.path,
            format!(r"{ROOT}\switch\{}", "a".repeat(259 - ROOT.len() - 7))
        );
    }

    #[test]
    fn the_longest_path_is_reported() {
        let long = "b".repeat(300);
        let longer = "c".repeat(400);
        let error = check(ROOT, '\\', ["short", long.as_str(), longer.as_str()], 259).unwrap_err();
        assert_eq!(error.path, format!(r"{ROOT}\{longer}"));
    }

    #[test]
    fn non_latin_letters_count_in_utf8_bytes() {
        let entry = "д".repeat(110);
        assert!(ROOT.len() + 1 + 110 < 259);
        let error = check(ROOT, '\\', [entry.as_str()], 259).unwrap_err();
        assert_eq!(error.length, ROOT.len() + 1 + 220);
    }

    #[test]
    fn no_entries_fit() {
        assert_eq!(check(ROOT, '\\', [], 259), Ok(()));
    }
}
