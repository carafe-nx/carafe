//! Switch with DBI over MTP: the adapter of the [`Device`] port.
//!
//! On Windows it goes through Windows Portable Devices; on other systems the Switch is not visible yet.

#[cfg(windows)]
mod windows_device;
#[cfg(windows)]
mod wpd;

use carafe_core::TitleId;
use carafe_core::dbi::InstallTarget;
use carafe_core::ports::{AdapterError, Device, DeviceStatus, TransferProgress};

/// The connection label in the window.
pub const VIA: &str = "MTP";

/// The transfer progress step: an event is sent when this many more thousandths of the file have been transferred.
pub const PROGRESS_STEP_PERMILLE: u64 = 5;

/// A Switch that DBI exposes over USB as an MTP device.
pub struct MtpDevice;

impl Device for MtpDevice {
    fn status(&self) -> DeviceStatus {
        let connected = platform::is_connected();
        DeviceStatus {
            connected,
            via: connected.then(|| VIA.to_owned()),
        }
    }

    fn install(
        &self,
        nsp_path: &str,
        target: InstallTarget,
        progress: &mut dyn FnMut(TransferProgress),
    ) -> Result<(), AdapterError> {
        platform::install(nsp_path, target, progress)
    }

    fn fetch_logs(&self, title_id: TitleId, dest_dir: &str) -> Result<usize, AdapterError> {
        platform::fetch_logs(title_id, dest_dir)
    }
}

/// Returns the storage names of the connected Switch — to check what the adapter sees.
///
/// # Errors
///
/// [`AdapterError::Device`] if the Switch is not visible or did not respond.
pub fn storage_names() -> Result<Vec<String>, AdapterError> {
    platform::storage_names()
}

#[cfg(windows)]
use windows_device as platform;

#[cfg(not(windows))]
mod platform {
    use carafe_core::TitleId;
    use carafe_core::dbi::InstallTarget;
    use carafe_core::ports::{AdapterError, TransferProgress};

    const UNSUPPORTED: &str = "MTP is supported on Windows only for now";

    pub fn is_connected() -> bool {
        false
    }

    pub fn install(
        _nsp_path: &str,
        _target: InstallTarget,
        _progress: &mut dyn FnMut(TransferProgress),
    ) -> Result<(), AdapterError> {
        Err(AdapterError::Device(UNSUPPORTED.to_owned()))
    }

    pub fn fetch_logs(_title_id: TitleId, _dest_dir: &str) -> Result<usize, AdapterError> {
        Err(AdapterError::Device(UNSUPPORTED.to_owned()))
    }

    pub fn storage_names() -> Result<Vec<String>, AdapterError> {
        Err(AdapterError::Device(UNSUPPORTED.to_owned()))
    }
}

/// Skips progress events while fewer than [`PROGRESS_STEP_PERMILLE`] thousandths were transferred since the last one.
pub struct Throttle {
    total: u64,
    reported: Option<u64>,
}

impl Throttle {
    /// Creates a filter for a file of `total` bytes.
    #[must_use]
    pub fn new(total: u64) -> Self {
        Self {
            total,
            reported: None,
        }
    }

    /// Returns whether to send an event for `done` transferred bytes: the first, one per step and the last.
    pub fn should_report(&mut self, done: u64) -> bool {
        let permille = done.min(self.total) * 1000 / self.total.max(1);
        let due = match self.reported {
            None => true,
            Some(last) => permille >= last + PROGRESS_STEP_PERMILLE || done >= self.total,
        };
        if due {
            self.reported = Some(permille);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_reported_in_steps_and_at_the_end() {
        let mut throttle = Throttle::new(1000);
        let reported: Vec<u64> = (0..=1000)
            .step_by(1)
            .filter(|done| throttle.should_report(*done))
            .collect();
        assert_eq!(reported.first(), Some(&0));
        assert_eq!(reported.last(), Some(&1000));
        assert_eq!(reported.len(), 201);
    }

    #[test]
    fn empty_files_report_once() {
        let mut throttle = Throttle::new(0);
        assert!(throttle.should_report(0));
    }
}
