//! List of Wine DLLs with checksums (Autorun `horizon-dlls/manifest.json`) and checking the runtime against it.

use serde::Deserialize;

/// Path to the DLL list from the runtime RomFS root.
pub const DLL_MANIFEST: &str = "switch/wine/horizon-dlls/manifest.json";

const WINE_ROOT: &str = "switch/wine";
const LISTED_LIMIT: usize = 10;

/// List of Wine DLLs; only the fields needed for checking are read.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DllManifest {
    /// Runtime files.
    pub files: Vec<DllEntry>,
}

/// Entry for a file in the DLL list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DllEntry {
    /// File name.
    pub name: String,
    /// Folder relative to `switch/wine`.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 in hex.
    pub sha256: String,
}

impl DllEntry {
    /// Returns the path to the file from the RomFS root, separated by `/`.
    #[must_use]
    pub fn romfs_path(&self) -> String {
        format!("{WINE_ROOT}/{}/{}", self.path.trim_matches('/'), self.name)
    }
}

/// What could be learned about a file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// The file does not exist.
    Missing,
    /// The file exists but cannot be read.
    Unreadable,
    /// The file was read in full.
    Read {
        /// Size in bytes.
        size: u64,
        /// SHA-256 in hex.
        sha256: String,
    },
}

/// What makes a runtime file unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Damage {
    /// The file does not exist: for example, an antivirus quarantined it.
    Missing,
    /// The file cannot be read: for example, an antivirus blocked opening it.
    Unreadable,
    /// The contents do not match the list.
    Changed,
}

/// Damaged runtime file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamagedFile {
    /// Path from the RomFS root.
    pub path: String,
    /// What is wrong with it.
    pub damage: Damage,
}

/// Checks a file against a list entry.
///
/// # Returns
///
/// `None` if the size and SHA-256 match (the case of hex digits does not matter); otherwise what is wrong.
#[must_use]
pub fn check(entry: &DllEntry, observed: &Observed) -> Option<Damage> {
    match observed {
        Observed::Missing => Some(Damage::Missing),
        Observed::Unreadable => Some(Damage::Unreadable),
        Observed::Read { size, sha256 }
            if *size == entry.size && sha256.eq_ignore_ascii_case(&entry.sha256) =>
        {
            None
        }
        Observed::Read { .. } => Some(Damage::Changed),
    }
}

/// Returns a list of damaged files for the log: the first ten paths, each marked with what is wrong with
/// the file, and how many more remain beyond the list.
#[must_use]
pub fn describe(damaged: &[DamagedFile]) -> String {
    let mut listed: Vec<String> = damaged
        .iter()
        .take(LISTED_LIMIT)
        .map(|file| {
            let what = match file.damage {
                Damage::Missing => "missing",
                Damage::Unreadable => "unreadable",
                Damage::Changed => "checksum mismatch",
            };
            format!("{} ({what})", file.path)
        })
        .collect();
    if damaged.len() > LISTED_LIMIT {
        listed.push(format!("+{} more", damaged.len() - LISTED_LIMIT));
    }
    listed.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> DllEntry {
        DllEntry {
            name: "d3d11.dll".to_owned(),
            path: "drive_c/windows/syswow64".to_owned(),
            size: 704_512,
            sha256: "ccac49f1832cbe9fb2aa0242f002844758bdcf825431508330edc064d932d3c6".to_owned(),
        }
    }

    #[test]
    fn manifest_entries_keep_only_needed_fields() {
        let json = r#"{"schema":2,"files":[{"name":"d3d11.dll","path":"drive_c/windows/syswow64",
            "arch":"i386","size":704512,
            "sha256":"ccac49f1832cbe9fb2aa0242f002844758bdcf825431508330edc064d932d3c6"}]}"#;
        let manifest: DllManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.files, vec![entry()]);
        assert_eq!(
            manifest.files[0].romfs_path(),
            "switch/wine/drive_c/windows/syswow64/d3d11.dll"
        );
    }

    #[test]
    fn matching_file_is_fine() {
        let observed = Observed::Read {
            size: 704_512,
            sha256: entry().sha256.to_uppercase(),
        };
        assert_eq!(check(&entry(), &observed), None);
    }

    #[test]
    fn missing_unreadable_and_changed_files_are_damaged() {
        assert_eq!(check(&entry(), &Observed::Missing), Some(Damage::Missing));
        assert_eq!(
            check(&entry(), &Observed::Unreadable),
            Some(Damage::Unreadable)
        );
        let other_size = Observed::Read {
            size: 1,
            sha256: entry().sha256,
        };
        assert_eq!(check(&entry(), &other_size), Some(Damage::Changed));
        let other_hash = Observed::Read {
            size: 704_512,
            sha256: "00".repeat(32),
        };
        assert_eq!(check(&entry(), &other_hash), Some(Damage::Changed));
    }

    #[test]
    fn long_lists_are_cut() {
        let damaged: Vec<DamagedFile> = (0..12)
            .map(|index| DamagedFile {
                path: format!("f{index}.dll"),
                damage: Damage::Missing,
            })
            .collect();
        let text = describe(&damaged);
        assert!(text.starts_with("f0.dll (missing); f1.dll (missing)"));
        assert!(text.ends_with("f9.dll (missing); +2 more"));
    }
}
