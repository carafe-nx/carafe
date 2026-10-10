//! Traits through which the core reaches the outside world, and the data they exchange.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::dbi::InstallTarget;
use crate::icon::IconJpeg;
use crate::record::{Arch, BuildRecord};
use crate::title_id::TitleId;

/// Adapter error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdapterError {
    /// The required file, folder or game does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// Read or write error.
    #[error("I/O error: {0}")]
    Io(String),
    /// An external program (hacBrewPack) failed.
    #[error("packing error: {0}")]
    Tool(String),
    /// An antivirus prevents reading files; the text says which.
    #[error("antivirus blocked files: {0}")]
    Blocked(String),
    /// Not enough disk space; the text says how much is needed and how much is free.
    #[error("not enough disk space: {0}")]
    NoSpace(String),
    /// A file path during packing is longer than hacBrewPack reads; the text has the longest path.
    #[error("path is longer than hacBrewPack reads: {0}")]
    PathTooLong(String),
    /// Runtime files are missing, unreadable or do not match their checksums.
    #[error("runtime is damaged: {0}")]
    RuntimeDamaged(String),
    /// An online service is unavailable or returned an error.
    #[error("network error: {0}")]
    Network(String),
    /// The service did not accept the API key.
    #[error("API key not accepted: {0}")]
    Unauthorized(String),
    /// The file has the wrong format.
    #[error("unsupported format: {0}")]
    Unsupported(String),
    /// The Switch is not connected or failed.
    #[error("device error: {0}")]
    Device(String),
}

/// NSP in the library folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StoredGame {
    /// Record from `carafe.json` inside the NSP.
    pub record: BuildRecord,
    /// NSP file name.
    pub file_name: String,
    /// NSP size in bytes.
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// The NSP's icon, if it could be read.
    #[ts(type = "Array<number> | null")]
    pub icon: Option<IconJpeg>,
}

/// Library NSPs.
pub trait GameStore: Send + Sync {
    /// Returns the Carafe NSPs from the `library_dir` folder: one per game, with the newest build.
    ///
    /// NSPs without `carafe.json`, corrupted ones and ones written by a newer version of Carafe are skipped.
    ///
    /// # Returns
    ///
    /// An empty list if the folder does not exist yet.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Io`] if the folder cannot be read.
    fn list(&self, library_dir: &str) -> Result<Vec<StoredGame>, AdapterError>;

    /// Deletes from the PC the game NSP that [`GameStore::list`] shows.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] if there is no such game; [`AdapterError::Io`] if deletion fails.
    fn delete(&self, library_dir: &str, title_id: TitleId) -> Result<(), AdapterError>;
}

/// Executable in the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecutableInfo {
    /// Path relative to the game folder.
    pub path: String,
    /// Bitness from the PE header.
    pub arch: Arch,
    /// The `.exe` runs only at its own address below 4 GB: it gets a 32-bit address space.
    pub fixed_address: bool,
    /// Size in bytes.
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// `ProductName` from the version resource.
    pub product_name: Option<String>,
    /// `CompanyName` from the version resource.
    pub company_name: Option<String>,
}

/// What was found in the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderReport {
    /// Game folder.
    pub folder: String,
    /// `.exe` files for x86 and x86-64 in the folder and subfolders.
    pub executables: Vec<ExecutableInfo>,
    /// There is a `steam_api.dll` or `steam_api64.dll`.
    pub has_steam_api: bool,
}

/// Game folder scanning.
pub trait FolderInspector: Send + Sync {
    /// Looks for `.exe` files and game information in the folder.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] or [`AdapterError::Io`] if the folder cannot be read;
    /// [`AdapterError::Unsupported`] if the folder has too many files.
    fn inspect(&self, folder: &str) -> Result<FolderReport, AdapterError>;
}

/// NSP build stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BuildStage {
    /// Unpacking the runtime and verifying checksums.
    Runtime,
    /// Copying the game into RomFS.
    Game,
    /// Metadata, icon, Autorun settings files.
    Metadata,
    /// Encryption and packing with hacBrewPack.
    Pack,
    /// Moving the NSP into the library and cleaning up.
    Finish,
}

/// Build progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildProgress {
    /// Current stage.
    pub stage: BuildStage,
    /// Stage completion, 0…100.
    pub percent: u8,
    /// Log line.
    pub log: Option<String>,
}

/// Image of a known format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFile {
    /// MIME type by the first bytes ([`crate::icon::image_mime`]).
    pub mime: &'static str,
    /// File contents.
    pub bytes: Vec<u8>,
}

/// Images from disk for the icon.
pub trait ImageReader: Send + Sync {
    /// Reads the user's image.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] or [`AdapterError::Io`] if the file cannot be read;
    /// [`AdapterError::Unsupported`] if it is not PNG, JPEG, GIF, WebP, BMP or ICO or the file is too large.
    fn read_image(&self, path: &str) -> Result<ImageFile, AdapterError>;

    /// Returns the largest icon from the `.exe` resources.
    ///
    /// # Returns
    ///
    /// `None` if the `.exe` has no icons.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] or [`AdapterError::Io`] if the file cannot be read;
    /// [`AdapterError::Unsupported`] if it is not a PE file or its resources are corrupted.
    fn executable_icon(&self, path: &str) -> Result<Option<ImageFile>, AdapterError>;
}

/// Game on SteamGridDB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtGame {
    /// Game number on SteamGridDB.
    pub id: u32,
    /// Title.
    pub name: String,
    /// Release year, if known.
    pub year: Option<i32>,
}

/// Square cover on SteamGridDB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ArtImage {
    /// Cover number.
    pub id: u32,
    /// URL of the full-size image.
    pub url: String,
    /// URL of the thumbnail for the list.
    pub thumb: String,
    /// Width, if the service reported it.
    pub width: Option<u32>,
    /// Height, if the service reported it.
    pub height: Option<u32>,
}

/// Cover search on SteamGridDB.
pub trait ArtSearch: Send + Sync {
    /// Searches for games by title.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Unauthorized`] if the key is not accepted; [`AdapterError::Network`] on a network error.
    fn search_games(&self, api_key: &str, term: &str) -> Result<Vec<ArtGame>, AdapterError>;

    /// Returns the game's square covers: 512×512 and 1024×1024, without animation.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Unauthorized`] if the key is not accepted; [`AdapterError::Network`] on a network error.
    fn square_images(&self, api_key: &str, game_id: u32) -> Result<Vec<ArtImage>, AdapterError>;

    /// Downloads a cover image.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Unsupported`] if the URL is not from the SteamGridDB image server or it is not an image;
    /// [`AdapterError::Network`] on a network error.
    fn download(&self, url: &str) -> Result<ImageFile, AdapterError>;
}

/// Keys and folder the NSP is built with; taken from the app settings at build time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackTarget {
    /// Path to `prod.keys`.
    pub keys_path: String,
    /// Library folder where the finished NSP goes.
    pub library_dir: String,
}

/// NSP building.
pub trait Packer: Send + Sync {
    /// Builds an NSP from the record and puts it into the library folder from `target`.
    ///
    /// `icon` is the icon for the Switch; `None` means the default runtime icon.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NoSpace`] if the library disk does not have enough space;
    /// [`AdapterError::PathTooLong`] if a path to a game file would be longer than the packer reads;
    /// [`AdapterError`] of another kind for other errors. An unfinished NSP does not get into the library.
    fn pack(
        &self,
        record: &BuildRecord,
        icon: Option<&IconJpeg>,
        target: &PackTarget,
        progress: &mut dyn FnMut(BuildProgress),
    ) -> Result<StoredGame, AdapterError>;

    /// Removes leftovers of interrupted builds from the library folder; builds in progress are not touched.
    ///
    /// # Returns
    ///
    /// The number of leftovers removed; zero if there is nothing to clean or the library folder does not exist.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Io`] if the library's service folder cannot be read.
    fn clean(&self, library_dir: &str) -> Result<usize, AdapterError>;
}

/// Switch connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceStatus {
    /// The Switch is visible over MTP.
    pub connected: bool,
    /// What it is visible through, for example "MTP".
    pub via: Option<String>,
}

/// Progress of a transfer to the Switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferProgress {
    /// Bytes transferred.
    #[ts(type = "number")]
    pub done_bytes: u64,
    /// Total bytes.
    #[ts(type = "number")]
    pub total_bytes: u64,
}

/// Switch connected over MTP.
pub trait Device: Send + Sync {
    /// Returns whether the Switch is visible.
    fn status(&self) -> DeviceStatus;

    /// Copies the NSP `nsp_path` to the DBI install storage for `target`.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the Switch is not visible, DBI lacks the required storage or the transfer broke off;
    /// [`AdapterError::NotFound`] or [`AdapterError::Io`] if the NSP cannot be read.
    fn install(
        &self,
        nsp_path: &str,
        target: InstallTarget,
        progress: &mut dyn FnMut(TransferProgress),
    ) -> Result<(), AdapterError>;

    /// Copies the game logs from the console memory card into the `dest_dir` folder, replacing its contents.
    ///
    /// # Returns
    ///
    /// The number of files copied.
    ///
    /// # Errors
    ///
    /// [`AdapterError::Device`] if the Switch is not visible; [`AdapterError::NotFound`] if there are no logs
    /// for this game on the card; [`AdapterError::Io`] if writing to the PC fails.
    fn fetch_logs(&self, title_id: TitleId, dest_dir: &str) -> Result<usize, AdapterError>;
}

/// Which of the required keys were found in `prod.keys`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeysReport {
    /// `header_key` was found.
    pub header_key: bool,
    /// `key_area_key_application_00` was found.
    pub key_area_key: bool,
}

impl KeysReport {
    /// Returns whether there are enough keys for packing.
    #[must_use]
    pub fn is_complete(self) -> bool {
        self.header_key && self.key_area_key
    }
}

/// Keys file check.
pub trait KeysChecker: Send + Sync {
    /// Checks whether the file contains the keys hacBrewPack needs.
    ///
    /// # Errors
    ///
    /// [`AdapterError::NotFound`] or [`AdapterError::Io`] if the file cannot be read.
    fn check(&self, path: &str) -> Result<KeysReport, AdapterError>;
}
