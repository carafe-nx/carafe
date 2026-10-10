//! Updating Carafe itself: checks, the background download and the installer launch.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use carafe_core::update::{
    CheckOutcome, FailedStep, Release, StartAfterInstall, UpdateState, after_check,
    start_after_install,
};
use carafe_core::version::{Version, runtime_outdated};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use ts_rs::TS;

use crate::error::{CommandError, ErrorCode};
use crate::preferences;
use crate::services::Services;

/// Event with the new [`UpdateStatus`].
pub const UPDATE_STATUS: &str = "update://status";

const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_INTERVAL: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const PROGRESS_STEP: Duration = Duration::from_millis(250);
const UPDATES_DIR: &str = "updates";
const MARKER_FILE: &str = "update.json";

/// What the window knows about updating Carafe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateStatus {
    /// This build can update itself; development builds cannot.
    pub enabled: bool,
    /// Where the update stands.
    pub state: UpdateState,
    /// A build or a USB install is running: installing now would interrupt it.
    pub busy: bool,
    /// The update is installed when Carafe closes.
    pub install_on_close: bool,
    /// The update is installed, and Carafe restarts, as soon as the running work finishes.
    pub install_when_idle: bool,
    /// When the last check finished, in milliseconds since the Unix epoch.
    #[ts(type = "number | null")]
    pub checked_at: Option<u64>,
    /// The version Carafe was updated to before this start.
    #[ts(type = "string | null")]
    pub updated_to: Option<Version>,
}

/// The update of Carafe; stored in `tauri::State`.
pub struct Updates {
    inner: Mutex<Inner>,
}

struct Inner {
    status: UpdateStatus,
    work: usize,
    update: Option<Update>,
    installer: Option<PathBuf>,
    download: Option<JoinHandle<()>>,
}

/// Running work that installing an update would interrupt; finishes when dropped.
pub struct Work {
    app: AppHandle,
}

#[derive(Serialize, Deserialize)]
struct Marker {
    version: Version,
}

impl Updates {
    /// Creates the update state; `enabled` is whether this build may update itself.
    pub fn new(enabled: bool) -> Self {
        let status = UpdateStatus {
            enabled,
            state: UpdateState::Idle,
            busy: false,
            install_on_close: false,
            install_when_idle: false,
            checked_at: None,
            updated_to: None,
        };
        Self {
            inner: Mutex::new(Inner {
                status,
                work: 0,
                update: None,
                installer: None,
                download: None,
            }),
        }
    }

    /// Returns the current status.
    pub fn status(&self) -> UpdateStatus {
        self.lock().status.clone()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Returns whether this build may update itself: a release build with a release version.
pub fn enabled(app: &AppHandle) -> bool {
    !cfg!(debug_assertions)
        && running_version(app).is_some_and(|version| version.to_string() != "0.0.0")
}

/// Reads what the previous install left behind, removes old installer copies and starts the
/// regular checks.
pub fn start(app: &AppHandle) {
    let updates = app.state::<Updates>();
    if !updates.status().enabled {
        return;
    }
    if let (Some(requested), Some(running)) = (take_marker(app), running_version(app)) {
        let mut inner = updates.lock();
        match start_after_install(requested, running) {
            StartAfterInstall::Updated(version) => inner.status.updated_to = Some(version),
            StartAfterInstall::NotInstalled(version) => {
                let release = Release {
                    version,
                    runtime_changed: false,
                    size_bytes: None,
                };
                inner.status.state = UpdateState::Failed {
                    release,
                    step: FailedStep::Install,
                };
            }
        }
    }
    let cleaner = app.clone();
    tauri::async_runtime::spawn_blocking(move || remove_leftovers(&cleaner));
    let scheduler = app.clone();
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK);
        loop {
            let mode = scheduler.state::<Services>().preferences().updates;
            if mode.checks_on_its_own() {
                let _ = tauri::async_runtime::block_on(check(&scheduler, false));
            }
            thread::sleep(CHECK_INTERVAL);
        }
    });
}

/// Looks for a newer Carafe and acts on what it finds as the update mode says; `asked` is whether the
/// player asked for this check.
///
/// # Errors
///
/// [`CommandError`] of kind `network` if GitHub did not answer and the player asked for the check; a
/// check Carafe made on its own fails silently.
pub async fn check(app: &AppHandle, asked: bool) -> Result<UpdateStatus, CommandError> {
    let updates = app.state::<Updates>();
    if !updates.status().enabled {
        return Ok(updates.status());
    }
    let found = match find(app).await {
        Ok(found) => found,
        Err(error) if asked => return Err(error),
        Err(_) => return Ok(updates.status()),
    };
    let release = found.as_ref().and_then(|update| release_of(app, update));
    let (mode, skipped) = {
        let services = app.state::<Services>();
        let preferences = services.preferences();
        (preferences.updates, preferences.skipped_update)
    };
    let to_download = {
        let mut inner = updates.lock();
        inner.status.checked_at = Some(now_ms());
        match after_check(&inner.status.state, release, mode, skipped, asked) {
            CheckOutcome::Keep => None,
            CheckOutcome::Show(state) => {
                if matches!(state, UpdateState::Idle) {
                    inner.installer = None;
                }
                inner.status.state = state;
                inner.update = found;
                None
            }
            CheckOutcome::Download(release) => {
                inner.update = found;
                Some(release)
            }
        }
    };
    match to_download {
        Some(release) => start_download(app, release),
        None => publish(app),
    }
    Ok(updates.status())
}

/// Starts downloading the known new version; if none is known, checks first.
///
/// # Errors
///
/// [`CommandError`] of kind `network` if the check fails.
pub async fn download(app: &AppHandle) -> Result<(), CommandError> {
    let release = {
        let updates = app.state::<Updates>();
        let inner = updates.lock();
        match (&inner.status.state, &inner.update) {
            (UpdateState::Available { release } | UpdateState::Failed { release, .. }, Some(_)) => {
                Some(release.clone())
            }
            _ => None,
        }
    };
    match release {
        Some(release) => start_download(app, release),
        None => {
            check(app, true).await?;
            let release = match &app.state::<Updates>().status().state {
                UpdateState::Available { release } => Some(release.clone()),
                _ => None,
            };
            if let Some(release) = release {
                start_download(app, release);
            }
        }
    }
    Ok(())
}

/// Stops the running download; the version is shown as available again.
pub fn cancel_download(app: &AppHandle) {
    {
        let updates = app.state::<Updates>();
        let mut inner = updates.lock();
        if let Some(task) = inner.download.take() {
            task.abort();
        }
        if let UpdateState::Downloading { release, .. } = &inner.status.state {
            inner.status.state = UpdateState::Available {
                release: release.clone(),
            };
        }
    }
    publish(app);
}

/// Hides the available version until a newer one is out.
///
/// # Errors
///
/// [`CommandError`] of kind `io` if the settings cannot be saved.
pub fn skip(app: &AppHandle) -> Result<(), CommandError> {
    let version = match app.state::<Updates>().status().state.release() {
        Some(release) => release.version,
        None => return Ok(()),
    };
    let services = app.state::<Services>();
    let mut next = services.preferences().clone();
    next.skipped_update = Some(version);
    if let Some(file) = services.preferences_file() {
        preferences::store(file, &next).map_err(|error| CommandError {
            code: ErrorCode::Io,
            message: error.to_string(),
        })?;
    }
    *services.preferences() = next;
    {
        let updates = app.state::<Updates>();
        let mut inner = updates.lock();
        inner.status.state = UpdateState::Idle;
        inner.installer = None;
    }
    publish(app);
    Ok(())
}

/// Sets whether the downloaded update is installed when Carafe closes.
pub fn set_install_on_close(app: &AppHandle, enabled: bool) {
    app.state::<Updates>().lock().status.install_on_close = enabled;
    publish(app);
}

/// Sets whether the downloaded update is installed as soon as the running work finishes; with no work
/// running it is installed now.
///
/// # Errors
///
/// [`CommandError`] as for [`install`] when it is installed now.
pub fn set_install_when_idle(app: &AppHandle, enabled: bool) -> Result<(), CommandError> {
    let idle = {
        let updates = app.state::<Updates>();
        let mut inner = updates.lock();
        inner.status.install_when_idle = enabled;
        inner.work == 0
    };
    publish(app);
    if enabled && idle {
        install(app, true)?;
    }
    Ok(())
}

/// Runs the downloaded installer: Carafe closes and, if `restart`, opens again on the new version.
///
/// Returns only if the installer could not be started.
///
/// # Errors
///
/// [`CommandError`] of kind `busy` while a build or a USB install is running, `notFound` if no update
/// is downloaded, `io` if the installer cannot be read or started.
pub fn install(app: &AppHandle, restart: bool) -> Result<(), CommandError> {
    if app.state::<Updates>().lock().work > 0 {
        return Err(CommandError {
            code: ErrorCode::Busy,
            message: "a build or a USB install is running".to_owned(),
        });
    }
    run_installer(app, restart)
}

/// Installs the downloaded update if the player asked for it to happen when Carafe closes.
pub fn on_exit(app: &AppHandle) {
    let wanted = {
        let updates = app.state::<Updates>();
        let inner = updates.lock();
        inner.status.install_on_close && matches!(inner.status.state, UpdateState::Ready { .. })
    };
    if wanted {
        let _ = run_installer(app, false);
    }
}

fn run_installer(app: &AppHandle, restart: bool) -> Result<(), CommandError> {
    let (update, installer, release) = {
        let updates = app.state::<Updates>();
        let inner = updates.lock();
        match (&inner.status.state, &inner.update, &inner.installer) {
            (UpdateState::Ready { release }, Some(update), Some(installer)) => {
                (update.clone(), installer.clone(), release.clone())
            }
            _ => {
                return Err(CommandError {
                    code: ErrorCode::NotFound,
                    message: "no update is downloaded".to_owned(),
                });
            }
        }
    };
    let result = fs::read(&installer)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            write_marker(app, release.version)?;
            update
                .restart_after_install(restart)
                .install(bytes)
                .map_err(|error| error.to_string())
        });
    let message = match result {
        Ok(()) => return Ok(()),
        Err(message) => message,
    };
    let _ = take_marker(app);
    {
        let updates = app.state::<Updates>();
        let mut inner = updates.lock();
        inner.status.state = UpdateState::Failed {
            release,
            step: FailedStep::Install,
        };
        inner.status.install_on_close = false;
        inner.status.install_when_idle = false;
    }
    publish(app);
    Err(CommandError {
        code: ErrorCode::Io,
        message,
    })
}

/// Marks the start of work that installing an update would interrupt.
pub fn begin_work(app: &AppHandle) -> Work {
    {
        let updates = app.state::<Updates>();
        let mut inner = updates.lock();
        inner.work += 1;
        inner.status.busy = true;
    }
    publish(app);
    Work { app: app.clone() }
}

impl Drop for Work {
    fn drop(&mut self) {
        let install_now = {
            let updates = self.app.state::<Updates>();
            let mut inner = updates.lock();
            inner.work = inner.work.saturating_sub(1);
            inner.status.busy = inner.work > 0;
            inner.work == 0
                && inner.status.install_when_idle
                && matches!(inner.status.state, UpdateState::Ready { .. })
        };
        publish(&self.app);
        if install_now {
            let _ = install(&self.app, true);
        }
    }
}

async fn find(app: &AppHandle) -> Result<Option<Update>, CommandError> {
    let network = |error: tauri_plugin_updater::Error| CommandError {
        code: ErrorCode::Network,
        message: error.to_string(),
    };
    let updater = app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .build()
        .map_err(network)?;
    updater.check().await.map_err(network)
}

fn release_of(app: &AppHandle, update: &Update) -> Option<Release> {
    let version = Version::parse(&update.version).ok()?;
    let current = &app.state::<Services>().runtime_version;
    let runtime_changed = update
        .raw_json
        .get("runtime")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|runtime| runtime_outdated(current, runtime));
    let size_bytes = tauri_plugin_updater::target().and_then(|target| {
        update
            .raw_json
            .get("platforms")?
            .get(target)?
            .get("size")?
            .as_u64()
    });
    Some(Release {
        version,
        runtime_changed,
        size_bytes,
    })
}

fn start_download(app: &AppHandle, release: Release) {
    let update = {
        let updates = app.state::<Updates>();
        let mut inner = updates.lock();
        let Some(update) = inner.update.clone() else {
            return;
        };
        if let Some(task) = inner.download.take() {
            task.abort();
        }
        inner.installer = None;
        inner.status.state = UpdateState::Downloading {
            release: release.clone(),
            received: 0,
            total: release.size_bytes,
        };
        update
    };
    publish(app);
    let worker = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let state = match fetch(&worker, update, &release).await {
            Ok(installer) => {
                worker.state::<Updates>().lock().installer = Some(installer);
                UpdateState::Ready { release }
            }
            Err(step) => UpdateState::Failed { release, step },
        };
        {
            let updates = worker.state::<Updates>();
            let mut inner = updates.lock();
            inner.status.state = state;
            inner.download = None;
        }
        publish(&worker);
    });
    app.state::<Updates>().lock().download = Some(task);
}

async fn fetch(
    app: &AppHandle,
    mut update: Update,
    release: &Release,
) -> Result<PathBuf, FailedStep> {
    update.timeout = Some(DOWNLOAD_TIMEOUT);
    let mut received: u64 = 0;
    let mut shown = Instant::now();
    let bytes = update
        .download(
            |chunk, total| {
                received += chunk as u64;
                if shown.elapsed() >= PROGRESS_STEP {
                    shown = Instant::now();
                    app.state::<Updates>().lock().status.state = UpdateState::Downloading {
                        release: release.clone(),
                        received,
                        total,
                    };
                    publish(app);
                }
            },
            || {},
        )
        .await
        .map_err(|error| match error {
            tauri_plugin_updater::Error::Minisign(_)
            | tauri_plugin_updater::Error::Base64(_)
            | tauri_plugin_updater::Error::SignatureUtf8(_)
            | tauri_plugin_updater::Error::SignedVersionMismatch { .. }
            | tauri_plugin_updater::Error::MissingSignedVersion => FailedStep::Verify,
            _ => FailedStep::Download,
        })?;
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| FailedStep::Download)?
        .join(UPDATES_DIR);
    let file = dir.join(format!("Carafe-{}-setup.exe", release.version));
    let target = file.clone();
    tauri::async_runtime::spawn_blocking(move || {
        fs::create_dir_all(&dir)?;
        fs::write(&target, bytes)
    })
    .await
    .map_err(|_| FailedStep::Download)?
    .map_err(|_| FailedStep::Download)?;
    Ok(file)
}

fn publish(app: &AppHandle) {
    let _ = app.emit(UPDATE_STATUS, app.state::<Updates>().status());
}

fn running_version(app: &AppHandle) -> Option<Version> {
    Version::parse(&app.package_info().version.to_string()).ok()
}

fn marker_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_local_data_dir()
        .ok()
        .map(|dir| dir.join(MARKER_FILE))
}

fn write_marker(app: &AppHandle, version: Version) -> Result<(), String> {
    let file = marker_file(app).ok_or("no application data folder")?;
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_vec(&Marker { version }).map_err(|error| error.to_string())?;
    fs::write(file, text).map_err(|error| error.to_string())
}

fn take_marker(app: &AppHandle) -> Option<Version> {
    let file = marker_file(app)?;
    let text = fs::read(&file).ok()?;
    let _ = fs::remove_file(&file);
    serde_json::from_slice::<Marker>(&text)
        .ok()
        .map(|marker| marker.version)
}

fn remove_leftovers(app: &AppHandle) {
    if let Ok(dir) = app.path().app_local_data_dir() {
        let _ = fs::remove_dir_all(dir.join(UPDATES_DIR));
    }
    let prefix = format!("{}-", app.package_info().name);
    remove_updater_temp(&std::env::temp_dir(), &prefix);
}

fn remove_updater_temp(temp: &Path, prefix: &str) {
    let Ok(entries) = fs::read_dir(temp) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let ours = name.starts_with(prefix) && name.contains("-updater-");
        if ours && entry.path().is_dir() {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_carafe_updater_folders_are_removed() {
        let temp = std::env::temp_dir().join(format!("carafe-updates-{}", std::process::id()));
        let ours = temp.join("Carafe-0.3.0-updater-AbC123");
        let other_app = temp.join("Other-0.3.0-updater-AbC123");
        let not_updater = temp.join("Carafe-build");
        for dir in [&ours, &other_app, &not_updater] {
            fs::create_dir_all(dir).expect("created");
        }
        remove_updater_temp(&temp, "Carafe-");
        let left = (ours.exists(), other_app.exists(), not_updater.exists());
        fs::remove_dir_all(&temp).expect("removed");
        assert_eq!(left, (false, true, true));
    }
}
