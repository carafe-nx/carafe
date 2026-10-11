//! The wizard's build: one at a time, never together with the rebuild queue, and cancellable.

use std::sync::{Mutex, MutexGuard, PoisonError};

use carafe_core::TitleId;
use carafe_core::cancel::CancelToken;
use carafe_core::ports::{AdapterError, BuildStage};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::error::{CommandError, ErrorCode};
use crate::rebuilds::Rebuilds;

/// Event with the new [`BuildState`].
pub const BUILD_STATUS: &str = "build://status";

/// Where the wizard's build stands.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum BuildState {
    /// No build has run, or its result was closed.
    #[default]
    Idle,
    /// A game is being built.
    #[serde(rename_all = "camelCase")]
    Running {
        /// The game.
        title_id: TitleId,
        /// Its title.
        title: String,
        /// Current stage.
        stage: BuildStage,
        /// Stage completion, 0…100.
        percent: u8,
        /// The player asked to cancel, and the build has not stopped yet.
        cancelling: bool,
    },
    /// The game is built and in the library.
    #[serde(rename_all = "camelCase")]
    Done {
        /// The game.
        title_id: TitleId,
        /// Its title.
        title: String,
    },
    /// The build failed; a previous NSP of the game is kept.
    #[serde(rename_all = "camelCase")]
    Failed {
        /// The game.
        title_id: TitleId,
        /// Its title.
        title: String,
        /// Why.
        error: CommandError,
    },
    /// The player cancelled the build; its files are removed and a previous NSP of the game is kept.
    #[serde(rename_all = "camelCase")]
    Cancelled {
        /// The game.
        title_id: TitleId,
        /// Its title.
        title: String,
    },
}

/// The build slot: what runs and how to stop it, without Tauri.
#[derive(Debug, Default)]
pub struct BuildSlot {
    state: BuildState,
    cancel: Option<CancelToken>,
}

impl BuildSlot {
    /// Returns the current state.
    #[must_use]
    pub fn state(&self) -> &BuildState {
        &self.state
    }

    /// Returns whether a build is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        matches!(self.state, BuildState::Running { .. })
    }

    /// Takes the slot for the game and returns the token its build checks.
    ///
    /// # Errors
    ///
    /// [`CommandError`] of kind `busy` if a build is already running or `rebuild_running` is true.
    pub fn start(
        &mut self,
        title_id: TitleId,
        title: String,
        rebuild_running: bool,
    ) -> Result<CancelToken, CommandError> {
        if self.is_running() {
            return Err(busy("a build is already running"));
        }
        if rebuild_running {
            return Err(busy("a rebuild is running"));
        }
        let cancel = CancelToken::new();
        self.state = BuildState::Running {
            title_id,
            title,
            stage: BuildStage::Runtime,
            percent: 0,
            cancelling: false,
        };
        self.cancel = Some(cancel.clone());
        Ok(cancel)
    }

    /// Records the running build's progress; does nothing if no build runs.
    pub fn progress(&mut self, new_stage: BuildStage, new_percent: u8) {
        if let BuildState::Running { stage, percent, .. } = &mut self.state {
            *stage = new_stage;
            *percent = new_percent;
        }
    }

    /// Asks the running build to stop.
    ///
    /// # Returns
    ///
    /// `false` if no build runs, so there is nothing to cancel.
    pub fn cancel(&mut self) -> bool {
        let BuildState::Running { cancelling, .. } = &mut self.state else {
            return false;
        };
        *cancelling = true;
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        true
    }

    /// Records how the running build ended and frees the slot; does nothing if no build runs.
    ///
    /// A build that finished before it saw the cancel is `Done`; an error caused by the cancel is `Cancelled`.
    pub fn finish(&mut self, result: Result<(), AdapterError>) {
        let BuildState::Running {
            title_id, title, ..
        } = &self.state
        else {
            return;
        };
        let (title_id, title) = (*title_id, title.clone());
        self.state = match result {
            Ok(()) => BuildState::Done { title_id, title },
            Err(AdapterError::Cancelled) => BuildState::Cancelled { title_id, title },
            Err(error) => BuildState::Failed {
                title_id,
                title,
                error: error.into(),
            },
        };
        self.cancel = None;
    }

    /// Closes a finished build's result; a running build is not touched.
    pub fn dismiss(&mut self) {
        if !self.is_running() {
            self.state = BuildState::Idle;
        }
    }
}

/// The build slot; stored in `tauri::State`.
#[derive(Default)]
pub struct Builds {
    slot: Mutex<BuildSlot>,
}

impl Builds {
    /// Returns the current state.
    pub fn state(&self) -> BuildState {
        self.lock().state().clone()
    }

    /// Locks the slot; the rebuild queue holds it while it starts, so a build and a rebuild never start together.
    pub fn lock(&self) -> MutexGuard<'_, BuildSlot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Takes the slot for the game and publishes the new state.
///
/// # Errors
///
/// [`CommandError`] of kind `busy` if a build or a rebuild is running.
pub fn start(
    app: &AppHandle,
    title_id: TitleId,
    title: String,
) -> Result<CancelToken, CommandError> {
    let cancel = {
        let builds = app.state::<Builds>();
        let mut slot = builds.lock();
        let rebuild_running = app.state::<Rebuilds>().status().running;
        slot.start(title_id, title, rebuild_running)?
    };
    publish(app);
    Ok(cancel)
}

/// Records the running build's progress and publishes it.
pub fn progress(app: &AppHandle, stage: BuildStage, percent: u8) {
    app.state::<Builds>().lock().progress(stage, percent);
    publish(app);
}

/// Asks the running build to stop and publishes the state.
pub fn cancel(app: &AppHandle) {
    if app.state::<Builds>().lock().cancel() {
        publish(app);
    }
}

/// Records how the build ended and publishes it.
pub fn finish(app: &AppHandle, result: Result<(), AdapterError>) {
    app.state::<Builds>().lock().finish(result);
    publish(app);
}

/// Closes a finished build's result and publishes the state.
pub fn dismiss(app: &AppHandle) {
    app.state::<Builds>().lock().dismiss();
    publish(app);
}

fn publish(app: &AppHandle) {
    let _ = app.emit(BUILD_STATUS, app.state::<Builds>().state());
}

fn busy(message: &str) -> CommandError {
    CommandError {
        code: ErrorCode::Busy,
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> TitleId {
        TitleId::parse("056694dd13640000").unwrap()
    }

    #[test]
    fn a_second_build_is_refused_while_one_runs() {
        let mut slot = BuildSlot::default();
        slot.start(game(), "OpenTTD".to_owned(), false).unwrap();
        let second = slot.start(game(), "OpenTTD".to_owned(), false);
        assert_eq!(second.map(|_| ()).unwrap_err().code, ErrorCode::Busy);
    }

    #[test]
    fn a_build_is_refused_while_a_rebuild_runs() {
        let mut slot = BuildSlot::default();
        let result = slot.start(game(), "OpenTTD".to_owned(), true);
        assert_eq!(result.map(|_| ()).unwrap_err().code, ErrorCode::Busy);
        assert_eq!(slot.state(), &BuildState::Idle);
    }

    #[test]
    fn cancel_reaches_the_build_and_ends_as_cancelled() {
        let mut slot = BuildSlot::default();
        let token = slot.start(game(), "OpenTTD".to_owned(), false).unwrap();
        assert!(slot.cancel());
        assert!(token.is_cancelled());
        assert!(matches!(
            slot.state(),
            BuildState::Running {
                cancelling: true,
                ..
            }
        ));
        slot.finish(Err(AdapterError::Cancelled));
        assert!(matches!(slot.state(), BuildState::Cancelled { .. }));
        assert!(slot.start(game(), "OpenTTD".to_owned(), false).is_ok());
    }

    #[test]
    fn a_build_that_finishes_before_the_cancel_stays_built() {
        let mut slot = BuildSlot::default();
        slot.start(game(), "OpenTTD".to_owned(), false).unwrap();
        slot.cancel();
        slot.finish(Ok(()));
        assert!(matches!(slot.state(), BuildState::Done { .. }));
    }

    #[test]
    fn a_failed_build_keeps_its_error() {
        let mut slot = BuildSlot::default();
        slot.start(game(), "OpenTTD".to_owned(), false).unwrap();
        slot.finish(Err(AdapterError::NoSpace("1 GB".to_owned())));
        let BuildState::Failed { error, .. } = slot.state() else {
            panic!("not failed: {:?}", slot.state());
        };
        assert_eq!(error.code, ErrorCode::NoSpace);
    }

    #[test]
    fn nothing_to_cancel_without_a_build() {
        let mut slot = BuildSlot::default();
        assert!(!slot.cancel());
        slot.finish(Ok(()));
        assert_eq!(slot.state(), &BuildState::Idle);
    }

    #[test]
    fn progress_updates_only_a_running_build() {
        let mut slot = BuildSlot::default();
        slot.progress(BuildStage::Pack, 50);
        assert_eq!(slot.state(), &BuildState::Idle);
        slot.start(game(), "OpenTTD".to_owned(), false).unwrap();
        slot.progress(BuildStage::Pack, 50);
        assert!(matches!(
            slot.state(),
            BuildState::Running {
                stage: BuildStage::Pack,
                percent: 50,
                ..
            }
        ));
    }

    #[test]
    fn dismiss_closes_a_result_but_not_a_running_build() {
        let mut slot = BuildSlot::default();
        slot.start(game(), "OpenTTD".to_owned(), false).unwrap();
        slot.dismiss();
        assert!(slot.is_running());
        slot.finish(Ok(()));
        slot.dismiss();
        assert_eq!(slot.state(), &BuildState::Idle);
    }
}
