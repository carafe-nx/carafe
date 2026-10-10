//! Rebuilding several games one after another with Carafe's runtime.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use carafe_core::TitleId;
use carafe_core::ports::{BuildProgress, BuildStage, PackTarget};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::error::{CommandError, ErrorCode};
use crate::services::Services;
use crate::updates;

/// Event with the new [`RebuildStatus`].
pub const REBUILD_STATUS: &str = "rebuild://status";

const PROGRESS_STEP: Duration = Duration::from_millis(200);

/// Where one game of the rebuild stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum RebuildItemState {
    /// Waits for its turn.
    Waiting,
    /// Is being built.
    Building {
        /// Current stage.
        stage: BuildStage,
        /// Stage completion, 0…100.
        percent: u8,
    },
    /// Built with the new runtime; the old NSP is replaced.
    Done,
    /// Not built; the old NSP is kept.
    Failed {
        /// Why.
        error: CommandError,
    },
    /// Not started because the player stopped the rebuild.
    Skipped,
}

/// One game of the rebuild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebuildItem {
    /// The game.
    pub title_id: TitleId,
    /// Its title.
    pub title: String,
    /// Where it stands.
    pub state: RebuildItemState,
}

/// The rebuild of several games.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RebuildStatus {
    /// The games in build order; empty if no rebuild was started.
    pub items: Vec<RebuildItem>,
    /// A game is being built or waits for its turn.
    pub running: bool,
    /// The player asked to stop after the current game.
    pub stopping: bool,
}

/// The rebuild of several games; stored in `tauri::State`.
#[derive(Default)]
pub struct Rebuilds {
    status: Mutex<RebuildStatus>,
}

impl Rebuilds {
    /// Returns the current status.
    pub fn status(&self) -> RebuildStatus {
        self.lock().clone()
    }

    fn lock(&self) -> MutexGuard<'_, RebuildStatus> {
        self.status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Starts rebuilding `games` one by one with the same settings, Title ID and icon; progress arrives as
/// [`REBUILD_STATUS`] events.
///
/// # Errors
///
/// [`CommandError`] of kind `busy` if a rebuild is already running, `invalid` if the settings lack the
/// path to `prod.keys` or the library folder or no game is given.
pub fn start(app: &AppHandle, games: Vec<TitleId>) -> Result<(), CommandError> {
    if games.is_empty() {
        return Err(CommandError::invalid("no games to rebuild"));
    }
    let services = app.state::<Services>();
    let target = {
        let preferences = services.preferences();
        PackTarget {
            keys_path: preferences
                .keys_path
                .clone()
                .ok_or_else(|| CommandError::invalid("no path to prod.keys set"))?,
            library_dir: preferences
                .library_dir
                .clone()
                .ok_or_else(|| CommandError::invalid("no library folder set"))?,
        }
    };
    let stored = services.games()?;
    let items = games
        .iter()
        .map(|&title_id| RebuildItem {
            title_id,
            title: stored
                .iter()
                .find(|game| game.record.title_id == title_id)
                .map(|game| game.record.metadata.title.clone())
                .unwrap_or_else(|| title_id.to_string()),
            state: RebuildItemState::Waiting,
        })
        .collect();
    {
        let rebuilds = app.state::<Rebuilds>();
        let mut status = rebuilds.lock();
        if status.running {
            return Err(CommandError {
                code: ErrorCode::Busy,
                message: "a rebuild is already running".to_owned(),
            });
        }
        *status = RebuildStatus {
            items,
            running: true,
            stopping: false,
        };
    }
    publish(app);
    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _work = updates::begin_work(&worker);
        for (index, &title_id) in games.iter().enumerate() {
            if worker.state::<Rebuilds>().lock().stopping {
                set_from(&worker, index, RebuildItemState::Skipped);
                break;
            }
            let state = match rebuild_one(&worker, title_id, index, &target) {
                Ok(()) => RebuildItemState::Done,
                Err(error) => RebuildItemState::Failed { error },
            };
            set(&worker, index, state);
        }
        worker.state::<Rebuilds>().lock().running = false;
        publish(&worker);
    });
    Ok(())
}

/// Asks the rebuild to stop after the current game.
pub fn stop(app: &AppHandle) {
    app.state::<Rebuilds>().lock().stopping = true;
    publish(app);
}

fn rebuild_one(
    app: &AppHandle,
    title_id: TitleId,
    index: usize,
    target: &PackTarget,
) -> Result<(), CommandError> {
    let services = app.state::<Services>();
    let game = services
        .store
        .list(&target.library_dir)?
        .into_iter()
        .find(|game| game.record.title_id == title_id)
        .ok_or_else(|| CommandError {
            code: ErrorCode::NotFound,
            message: format!("{title_id} is not in the library"),
        })?;
    let record = game.record.next_build(&services.runtime_version);
    let mut shown: Option<Instant> = None;
    let mut report = |progress: BuildProgress| {
        if shown.is_some_and(|at| at.elapsed() < PROGRESS_STEP) {
            return;
        }
        shown = Some(Instant::now());
        let state = RebuildItemState::Building {
            stage: progress.stage,
            percent: progress.percent,
        };
        set(app, index, state);
    };
    report(BuildProgress {
        stage: BuildStage::Runtime,
        percent: 0,
        log: None,
    });
    services
        .packer
        .pack(&record, game.icon.as_ref(), target, &mut report)?;
    Ok(())
}

fn set(app: &AppHandle, index: usize, state: RebuildItemState) {
    if let Some(item) = app.state::<Rebuilds>().lock().items.get_mut(index) {
        item.state = state;
    }
    publish(app);
}

fn set_from(app: &AppHandle, index: usize, state: RebuildItemState) {
    for item in app.state::<Rebuilds>().lock().items.iter_mut().skip(index) {
        item.state = state.clone();
    }
    publish(app);
}

fn publish(app: &AppHandle) {
    let _ = app.emit(REBUILD_STATUS, app.state::<Rebuilds>().status());
}
