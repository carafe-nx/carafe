//! Application services assembled in the composition root.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use carafe_core::ports::{
    AdapterError, ArtSearch, Device, FolderInspector, GameStore, ImageReader, KeysChecker, Packer,
    StoredGame,
};
use carafe_core::wizard::RuntimeCapabilities;

use crate::preferences::Preferences;

/// Everything the commands use; stored in `tauri::State`.
pub struct Services {
    /// Library NSPs.
    pub store: Arc<dyn GameStore>,
    /// Game folder inspection.
    pub inspector: Arc<dyn FolderInspector>,
    /// NSP building.
    pub packer: Arc<dyn Packer>,
    /// Switch over MTP.
    pub device: Arc<dyn Device>,
    /// `prod.keys` checking.
    pub keys: Arc<dyn KeysChecker>,
    /// Images and `.exe` icons for the game icon.
    pub images: Arc<dyn ImageReader>,
    /// Cover search on SteamGridDB.
    pub art: Arc<dyn ArtSearch>,
    /// Version of the runtime bundled with the application.
    pub runtime_version: String,
    /// What this runtime can do.
    pub capabilities: RuntimeCapabilities,
    preferences: Mutex<Preferences>,
    preferences_file: OnceLock<PathBuf>,
}

impl Services {
    /// Creates the services from the adapters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn GameStore>,
        inspector: Arc<dyn FolderInspector>,
        packer: Arc<dyn Packer>,
        device: Arc<dyn Device>,
        keys: Arc<dyn KeysChecker>,
        images: Arc<dyn ImageReader>,
        art: Arc<dyn ArtSearch>,
        runtime_version: impl Into<String>,
        capabilities: RuntimeCapabilities,
    ) -> Self {
        Self {
            store,
            inspector,
            packer,
            device,
            keys,
            images,
            art,
            runtime_version: runtime_version.into(),
            capabilities,
            preferences: Mutex::new(Preferences::default()),
            preferences_file: OnceLock::new(),
        }
    }

    /// Returns the application settings under a lock.
    pub fn preferences(&self) -> MutexGuard<'_, Preferences> {
        self.preferences
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Returns the library NSPs; an empty list if the library folder is not chosen yet.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Io`] if the folder is unreadable.
    pub fn games(&self) -> Result<Vec<StoredGame>, AdapterError> {
        match self.library_dir() {
            Some(dir) => self.store.list(&dir),
            None => Ok(Vec::new()),
        }
    }

    /// Returns the library folder from the settings.
    pub fn library_dir(&self) -> Option<String> {
        self.preferences().library_dir.clone()
    }

    /// Remembers the file that stores the settings; the second and later calls change nothing.
    pub fn set_preferences_file(&self, path: PathBuf) {
        let _ = self.preferences_file.set(path);
    }

    /// Returns the settings file, or `None` if the system provided no application settings folder.
    pub fn preferences_file(&self) -> Option<&Path> {
        self.preferences_file.get().map(PathBuf::as_path)
    }
}

/// Returns 64 random bits from the generator the standard library uses to seed hash tables.
pub fn entropy() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0);
    hasher.finish()
}
