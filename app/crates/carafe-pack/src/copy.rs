//! Copying folders with progress by bytes.

use std::fs;
use std::path::{Path, PathBuf};

use carafe_core::ports::AdapterError;

use crate::io_error;

/// Folder contents: subfolders and files with their sizes, paths relative to the root.
pub struct Tree {
    root: PathBuf,
    dirs: Vec<PathBuf>,
    files: Vec<(PathBuf, u64)>,
    total_bytes: u64,
}

impl Tree {
    /// Walks the whole folder `root`, following symbolic links.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] if the folder does not exist; [`AdapterError::Io`] if it cannot be read.
    pub fn scan(root: &Path) -> Result<Self, AdapterError> {
        let mut tree = Self {
            root: root.to_owned(),
            dirs: Vec::new(),
            files: Vec::new(),
            total_bytes: 0,
        };
        let metadata = fs::metadata(root).map_err(|error| io_error(root, &error))?;
        if !metadata.is_dir() {
            return Err(AdapterError::NotFound(format!(
                "{}: not a folder",
                root.display()
            )));
        }
        tree.visit(Path::new(""))?;
        Ok(tree)
    }

    /// Returns the number of files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Returns the size of all files in bytes.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Returns the paths of all subfolders and files relative to the root, separated by `/`.
    pub fn paths(&self) -> Vec<String> {
        self.dirs
            .iter()
            .chain(self.files.iter().map(|(file, _)| file))
            .map(|path| {
                path.components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect()
    }

    /// Copies the folder into `dest`, creating subfolders; files with the same name in `dest` are replaced.
    ///
    /// `on_percent` receives the progress 0…100 by bytes, each value at most once.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Blocked`] if the antivirus prevented reading the original; [`AdapterError::Io`] if a file
    /// cannot be read or written.
    pub fn copy_to(&self, dest: &Path, on_percent: &mut dyn FnMut(u8)) -> Result<(), AdapterError> {
        fs::create_dir_all(dest).map_err(|error| io_error(dest, &error))?;
        for dir in &self.dirs {
            let target = dest.join(dir);
            fs::create_dir_all(&target).map_err(|error| io_error(&target, &error))?;
        }
        let mut done = 0u64;
        let mut reported = None;
        for (file, size) in &self.files {
            let source = self.root.join(file);
            fs::copy(&source, dest.join(file)).map_err(|error| io_error(&source, &error))?;
            done += size;
            let percent = percent(done, self.total_bytes);
            if reported != Some(percent) {
                reported = Some(percent);
                on_percent(percent);
            }
        }
        if reported.is_none() {
            on_percent(100);
        }
        Ok(())
    }

    fn visit(&mut self, relative: &Path) -> Result<(), AdapterError> {
        let dir = self.root.join(relative);
        let entries = fs::read_dir(&dir).map_err(|error| io_error(&dir, &error))?;
        for entry in entries {
            let entry = entry.map_err(|error| io_error(&dir, &error))?;
            let path = entry.path();
            let metadata = fs::metadata(&path).map_err(|error| io_error(&path, &error))?;
            let child = relative.join(entry.file_name());
            if metadata.is_dir() {
                self.dirs.push(child.clone());
                self.visit(&child)?;
            } else {
                self.total_bytes += metadata.len();
                self.files.push((child, metadata.len()));
            }
        }
        Ok(())
    }
}

fn percent(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 100;
    }
    u8::try_from(done.min(total) * 100 / total).unwrap_or(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("carafe-copy-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn copies_nested_files_and_empty_folders() {
        let root = temp_dir("source");
        fs::create_dir_all(root.join("baseset/empty")).unwrap();
        fs::write(root.join("game.exe"), b"MZ").unwrap();
        fs::write(root.join("baseset/opengfx.tar"), vec![1; 300]).unwrap();
        let tree = Tree::scan(&root).unwrap();
        assert_eq!(tree.file_count(), 2);
        let dest = temp_dir("dest");
        let mut seen = Vec::new();
        tree.copy_to(&dest, &mut |percent| seen.push(percent))
            .unwrap();
        assert_eq!(
            fs::read(dest.join("baseset/opengfx.tar")).unwrap().len(),
            300
        );
        assert!(dest.join("baseset/empty").is_dir());
        assert_eq!(seen.last(), Some(&100));
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn paths_list_folders_and_files_with_forward_slashes() {
        let root = temp_dir("paths");
        fs::create_dir_all(root.join("baseset/empty")).unwrap();
        fs::write(root.join("baseset/opengfx.tar"), b"tar").unwrap();
        let mut paths = Tree::scan(&root).unwrap().paths();
        paths.sort();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(paths, ["baseset", "baseset/empty", "baseset/opengfx.tar"]);
    }

    #[test]
    fn a_missing_folder_is_not_found() {
        let missing = temp_dir("missing");
        assert!(matches!(
            Tree::scan(&missing),
            Err(AdapterError::NotFound(_))
        ));
    }
}
