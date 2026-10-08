//! Checking the runtime DLLs against their checksum list before a build.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::Path;

use carafe_core::ports::AdapterError;
use carafe_core::runtime_files::{
    DLL_MANIFEST, DamagedFile, DllManifest, Observed, check, describe,
};
use sha2::{Digest, Sha256};

const BUFFER_SIZE: usize = 1 << 20;

/// Checks every DLL from `horizon-dlls/manifest.json` in the runtime RomFS `romfs` against its size and SHA-256.
///
/// `on_percent` receives the progress 0…100 by bytes.
///
/// # Returns
///
/// The number of checked files.
///
/// # Errors
///
/// [`AdapterError::RuntimeDamaged`] with the list of files if some are missing, unreadable or changed,
/// and also if the list itself is missing or cannot be parsed.
pub fn verify_runtime(romfs: &Path, on_percent: &mut dyn FnMut(u8)) -> Result<usize, AdapterError> {
    let manifest_path = romfs.join(DLL_MANIFEST);
    let damaged_manifest = |reason: String| {
        AdapterError::RuntimeDamaged(format!("{}: {reason}", manifest_path.display()))
    };
    let bytes = fs::read(&manifest_path).map_err(|error| damaged_manifest(error.to_string()))?;
    let manifest: DllManifest =
        serde_json::from_slice(&bytes).map_err(|error| damaged_manifest(error.to_string()))?;
    let total: u64 = manifest.files.iter().map(|entry| entry.size).sum();
    let mut done = 0u64;
    let mut reported = None;
    let mut buffer = vec![0; BUFFER_SIZE];
    let mut damaged = Vec::new();
    for entry in &manifest.files {
        let path = entry.romfs_path();
        let observed = observe(&romfs.join(&path), &mut buffer);
        if let Some(damage) = check(entry, &observed) {
            damaged.push(DamagedFile { path, damage });
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
    Ok(manifest.files.len())
}

fn observe(path: &Path, buffer: &mut [u8]) -> Observed {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Observed::Missing,
        Err(_) => return Observed::Unreadable,
    };
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    loop {
        match file.read(buffer) {
            Ok(0) => break,
            Ok(read) => {
                hasher.update(&buffer[..read]);
                size += read as u64;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Observed::Unreadable,
        }
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    Observed::Read { size, sha256 }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use carafe_core::runtime_files::Damage;

    use super::*;

    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn runtime(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("carafe-verify-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let dlls = root.join("switch/wine/drive_c/windows/syswow64");
        fs::create_dir_all(&dlls).unwrap();
        for (file, contents) in files {
            fs::write(dlls.join(file), contents).unwrap();
        }
        let entries: Vec<String> = ["a.dll", "b.dll"]
            .iter()
            .map(|name| {
                format!(
                    r#"{{"name":"{name}","path":"drive_c/windows/syswow64","size":5,"sha256":"{HELLO_SHA256}"}}"#
                )
            })
            .collect();
        let manifest = root.join(DLL_MANIFEST);
        fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        fs::write(&manifest, format!(r#"{{"files":[{}]}}"#, entries.join(","))).unwrap();
        root
    }

    #[test]
    fn intact_runtime_passes() {
        let root = runtime("intact", &[("a.dll", b"hello"), ("b.dll", b"hello")]);
        let mut last = None;
        let checked = verify_runtime(&root, &mut |percent| last = Some(percent));
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(checked, Ok(2));
        assert_eq!(last, Some(100));
    }

    #[test]
    fn missing_and_changed_files_are_listed() {
        let root = runtime("damaged", &[("a.dll", b"HELLO")]);
        let result = verify_runtime(&root, &mut |_| {});
        fs::remove_dir_all(&root).unwrap();
        let expected = describe(&[
            DamagedFile {
                path: "switch/wine/drive_c/windows/syswow64/a.dll".to_owned(),
                damage: Damage::Changed,
            },
            DamagedFile {
                path: "switch/wine/drive_c/windows/syswow64/b.dll".to_owned(),
                damage: Damage::Missing,
            },
        ]);
        assert_eq!(result, Err(AdapterError::RuntimeDamaged(expected)));
    }

    #[test]
    fn a_missing_manifest_means_a_damaged_runtime() {
        let root = runtime("no-manifest", &[]);
        fs::remove_file(root.join(DLL_MANIFEST)).unwrap();
        let result = verify_runtime(&root, &mut |_| {});
        fs::remove_dir_all(&root).unwrap();
        assert!(matches!(result, Err(AdapterError::RuntimeDamaged(_))));
    }
}
