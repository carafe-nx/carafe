//! Tauri shell of the Carafe builder: the composition root.

mod commands;
mod error;
mod preferences;
mod services;
mod shell;

use std::path::PathBuf;
use std::sync::Arc;

use carafe_core::library::library_dir_for;
use carafe_core::wizard::RuntimeCapabilities;
use carafe_folder::DiskFolderInspector;
use carafe_images::FileImageReader;
use carafe_keys::FileKeysChecker;
use carafe_library::DiskGameStore;
use carafe_mtp::MtpDevice;
use carafe_pack::HacBrewPacker;
use carafe_steamgriddb::SteamGridDb;
use tauri::path::PathResolver;
use tauri::{Manager, Runtime};

use crate::preferences::PREFERENCES_FILE;
use crate::services::Services;

const RUNTIME_VERSION: &str = "0.1.0";
const RUNTIME_ARCHIVE: &str = "carafe-runtime.bin";
const DEV_RUNTIME_ARCHIVE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/resources/carafe-runtime.bin");
const HACBREWPACK: &str = "hacbrewpack";

/// Starts the application; if Tauri fails to start it, prints the error and exits the process with code 1.
pub fn run() {
    let services = Services::new(
        Arc::new(DiskGameStore),
        Arc::new(DiskFolderInspector),
        Arc::new(HacBrewPacker::new(runtime_archive(), sidecar(HACBREWPACK))),
        Arc::new(MtpDevice),
        Arc::new(FileKeysChecker),
        Arc::new(FileImageReader),
        Arc::new(SteamGridDb::new()),
        RUNTIME_VERSION,
        RuntimeCapabilities { stable_x64: false },
    );
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(services)
        .setup(|app| {
            let services = app.state::<Services>();
            let file = app
                .path()
                .app_config_dir()
                .ok()
                .map(|dir| dir.join(PREFERENCES_FILE));
            let mut preferences = services.preferences();
            if let Some(file) = file {
                if let Some(stored) = preferences::load(&file) {
                    *preferences = stored;
                }
                services.set_preferences_file(file);
            }
            if preferences.library_dir.is_none() {
                preferences.library_dir = default_library_dir(app.path());
            }
            if let Some(dir) = preferences.library_dir.clone() {
                let packer = Arc::clone(&services.packer);
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = packer.clean(&dir);
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::preferences::get_preferences,
            commands::preferences::save_preferences,
            commands::preferences::check_keys,
            commands::preferences::recommended_settings,
            commands::preferences::library_dir_for,
            commands::preferences::create_library_dir,
            commands::device::device_status,
            commands::library::list_games,
            commands::library::get_record,
            commands::library::delete_game,
            commands::library::install_game,
            commands::library::fetch_logs,
            commands::wizard::inspect_folder,
            commands::wizard::wizard_warnings,
            commands::wizard::start_build,
            commands::icons::read_image,
            commands::icons::executable_icon,
            commands::icons::art_games,
            commands::icons::art_images,
            commands::icons::art_download,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        eprintln!("Carafe failed to start: {error}");
        std::process::exit(1);
    }
}

/// Returns the path to the sidecar program `name` next to the application executable, where Tauri
/// places it from `bundle.externalBin`.
///
/// If the application path is unknown, returns the bare name: the system looks the program up.
fn sidecar(name: &str) -> PathBuf {
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&file)))
        .unwrap_or_else(|| PathBuf::from(file))
}

/// Returns the runtime archive: next to the application executable if it is there (that is where the
/// installer places it), otherwise the crate's `resources/carafe-runtime.bin` for development.
fn runtime_archive() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(RUNTIME_ARCHIVE)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(DEV_RUNTIME_ARCHIVE))
}

/// Returns the library folder the onboarding suggests by default: `Carafe` in the documents
/// folder, or in the home folder if there is no documents folder.
///
/// Returns `None` if the system reports neither.
fn default_library_dir<R: Runtime>(paths: &PathResolver<R>) -> Option<String> {
    let base = paths.document_dir().or_else(|_| paths.home_dir()).ok()?;
    Some(library_dir_for(&base).to_string_lossy().into_owned())
}
