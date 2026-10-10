//! Rules for updating Carafe itself: what a check leads to and what a start after an update means.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::version::Version;

/// How Carafe looks for new versions on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateMode {
    /// Checks and downloads in the background; installs only when asked.
    #[default]
    Automatic,
    /// Checks and shows that a version is out; downloads only when asked.
    NotifyOnly,
    /// Checks only when asked.
    Off,
}

impl UpdateMode {
    /// Returns whether Carafe checks for new versions without being asked.
    #[must_use]
    pub fn checks_on_its_own(self) -> bool {
        self != Self::Off
    }
}

/// A Carafe release newer than the running one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Release {
    /// Version of the release.
    #[ts(type = "string")]
    pub version: Version,
    /// The release carries a runtime newer than the running one: games are worth rebuilding.
    pub runtime_changed: bool,
    /// Size of the installer in bytes, if the release states it.
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
}

/// The step at which an update failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FailedStep {
    /// Downloading the installer.
    Download,
    /// Checking the installer's signature: the file is not the one the release announced.
    Verify,
    /// Running the installer.
    Install,
}

/// Where the update of Carafe stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateState {
    /// No newer version is known.
    Idle,
    /// A newer version is out and not downloaded.
    Available {
        /// The release.
        release: Release,
    },
    /// The installer is being downloaded.
    Downloading {
        /// The release.
        release: Release,
        /// Bytes received so far.
        #[ts(type = "number")]
        received: u64,
        /// Size of the installer, if the server reported it.
        #[ts(type = "number | null")]
        total: Option<u64>,
    },
    /// The installer is downloaded and its signature checked.
    Ready {
        /// The release.
        release: Release,
    },
    /// The update did not finish.
    Failed {
        /// The release.
        release: Release,
        /// Where it stopped.
        step: FailedStep,
    },
}

impl UpdateState {
    /// Returns the release the state is about; `None` for [`UpdateState::Idle`].
    #[must_use]
    pub fn release(&self) -> Option<&Release> {
        match self {
            Self::Idle => None,
            Self::Available { release }
            | Self::Downloading { release, .. }
            | Self::Ready { release }
            | Self::Failed { release, .. } => Some(release),
        }
    }
}

/// What to do after a check for a new version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Keep the current state.
    Keep,
    /// Show the state.
    Show(UpdateState),
    /// Start downloading the release.
    Download(Release),
}

/// Returns what a check that found `found` leads to.
///
/// A download in progress or a downloaded installer of the same version is kept. A version the player
/// skipped is not shown again unless they asked for the check (`asked`). In [`UpdateMode::Automatic`]
/// a found release is downloaded; otherwise it is shown as available.
#[must_use]
pub fn after_check(
    current: &UpdateState,
    found: Option<Release>,
    mode: UpdateMode,
    skipped: Option<Version>,
    asked: bool,
) -> CheckOutcome {
    let Some(release) = found else {
        return match current {
            UpdateState::Downloading { .. } => CheckOutcome::Keep,
            _ => CheckOutcome::Show(UpdateState::Idle),
        };
    };
    let same = current.release().map(|known| known.version) == Some(release.version);
    let in_hand = matches!(
        current,
        UpdateState::Downloading { .. } | UpdateState::Ready { .. }
    );
    if same && in_hand {
        return CheckOutcome::Keep;
    }
    if !asked && skipped == Some(release.version) {
        return CheckOutcome::Show(UpdateState::Idle);
    }
    match mode {
        UpdateMode::Automatic => CheckOutcome::Download(release),
        UpdateMode::NotifyOnly | UpdateMode::Off => {
            CheckOutcome::Show(UpdateState::Available { release })
        }
    }
}

/// What a start of Carafe after an install was requested means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartAfterInstall {
    /// Carafe now runs the version it was updating to, or a newer one.
    Updated(Version),
    /// Carafe still runs an older version: the installer did not finish.
    NotInstalled(Version),
}

/// Returns what starting `running` means after an install of `requested` was started.
#[must_use]
pub fn start_after_install(requested: Version, running: Version) -> StartAfterInstall {
    if running >= requested {
        StartAfterInstall::Updated(running)
    } else {
        StartAfterInstall::NotInstalled(requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn release(text: &str) -> Release {
        Release {
            version: version(text),
            runtime_changed: false,
            size_bytes: None,
        }
    }

    #[test]
    fn nothing_found_clears_the_button() {
        let current = UpdateState::Available {
            release: release("0.3.0"),
        };
        let outcome = after_check(&current, None, UpdateMode::Automatic, None, false);
        assert_eq!(outcome, CheckOutcome::Show(UpdateState::Idle));
    }

    #[test]
    fn nothing_found_keeps_a_running_download() {
        let current = UpdateState::Downloading {
            release: release("0.3.0"),
            received: 10,
            total: Some(20),
        };
        let outcome = after_check(&current, None, UpdateMode::Automatic, None, false);
        assert_eq!(outcome, CheckOutcome::Keep);
    }

    #[test]
    fn automatic_mode_downloads() {
        let outcome = after_check(
            &UpdateState::Idle,
            Some(release("0.3.0")),
            UpdateMode::Automatic,
            None,
            false,
        );
        assert_eq!(outcome, CheckOutcome::Download(release("0.3.0")));
    }

    #[test]
    fn notify_mode_shows_the_release() {
        let outcome = after_check(
            &UpdateState::Idle,
            Some(release("0.3.0")),
            UpdateMode::NotifyOnly,
            None,
            false,
        );
        let shown = UpdateState::Available {
            release: release("0.3.0"),
        };
        assert_eq!(outcome, CheckOutcome::Show(shown));
    }

    #[test]
    fn a_downloaded_installer_of_the_same_version_is_kept() {
        let current = UpdateState::Ready {
            release: release("0.3.0"),
        };
        let outcome = after_check(
            &current,
            Some(release("0.3.0")),
            UpdateMode::Automatic,
            None,
            true,
        );
        assert_eq!(outcome, CheckOutcome::Keep);
    }

    #[test]
    fn a_newer_release_replaces_a_downloaded_one() {
        let current = UpdateState::Ready {
            release: release("0.3.0"),
        };
        let outcome = after_check(
            &current,
            Some(release("0.3.1")),
            UpdateMode::Automatic,
            None,
            false,
        );
        assert_eq!(outcome, CheckOutcome::Download(release("0.3.1")));
    }

    #[test]
    fn a_failed_download_is_tried_again() {
        let current = UpdateState::Failed {
            release: release("0.3.0"),
            step: FailedStep::Download,
        };
        let outcome = after_check(
            &current,
            Some(release("0.3.0")),
            UpdateMode::Automatic,
            None,
            false,
        );
        assert_eq!(outcome, CheckOutcome::Download(release("0.3.0")));
    }

    #[test]
    fn a_skipped_version_stays_hidden() {
        let outcome = after_check(
            &UpdateState::Idle,
            Some(release("0.3.0")),
            UpdateMode::NotifyOnly,
            Some(version("0.3.0")),
            false,
        );
        assert_eq!(outcome, CheckOutcome::Show(UpdateState::Idle));
    }

    #[test]
    fn a_skipped_version_shows_when_asked() {
        let outcome = after_check(
            &UpdateState::Idle,
            Some(release("0.3.0")),
            UpdateMode::NotifyOnly,
            Some(version("0.3.0")),
            true,
        );
        let shown = UpdateState::Available {
            release: release("0.3.0"),
        };
        assert_eq!(outcome, CheckOutcome::Show(shown));
    }

    #[test]
    fn skipping_one_version_does_not_hide_the_next() {
        let outcome = after_check(
            &UpdateState::Idle,
            Some(release("0.3.1")),
            UpdateMode::NotifyOnly,
            Some(version("0.3.0")),
            false,
        );
        let shown = UpdateState::Available {
            release: release("0.3.1"),
        };
        assert_eq!(outcome, CheckOutcome::Show(shown));
    }

    #[test]
    fn off_mode_checks_only_when_asked() {
        assert!(!UpdateMode::Off.checks_on_its_own());
        assert!(UpdateMode::NotifyOnly.checks_on_its_own());
        assert!(UpdateMode::Automatic.checks_on_its_own());
    }

    #[test]
    fn start_on_the_new_version_is_an_update() {
        assert_eq!(
            start_after_install(version("0.3.0"), version("0.3.0")),
            StartAfterInstall::Updated(version("0.3.0"))
        );
    }

    #[test]
    fn start_on_the_old_version_is_a_failed_install() {
        assert_eq!(
            start_after_install(version("0.3.0"), version("0.2.0")),
            StartAfterInstall::NotInstalled(version("0.3.0"))
        );
    }
}
