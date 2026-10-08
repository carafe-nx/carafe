//! How a Switch running DBI looks over MTP: the device, storages, game logs on the SD card.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::title_id::TitleId;

const NINTENDO_VENDOR: &str = "vid_057e";
/// Name, without the number, of the DBI storage that shows the memory card files.
pub const SD_CARD_STORAGE: &str = "SD Card";
const SD_INSTALL: &str = "SD Card install";
const NAND_INSTALL: &str = "NAND install";
const LOGS_ROOT: [&str; 2] = ["switch", "carafe"];
const SESSION_SUFFIX: &str = ".shm";

/// Where DBI installs the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallTarget {
    /// Memory card.
    Sd,
    /// Internal console memory.
    Nand,
}

impl InstallTarget {
    /// Returns the name, without the number, of the DBI storage that installs the game here when copied to.
    #[must_use]
    pub fn storage(self) -> &'static str {
        match self {
            Self::Sd => SD_INSTALL,
            Self::Nand => NAND_INSTALL,
        }
    }
}

/// Returns whether a device PnP identifier looks like a Switch: it contains the Nintendo vendor code.
#[must_use]
pub fn is_switch(device_id: &str) -> bool {
    device_id.to_ascii_lowercase().contains(NINTENDO_VENDOR)
}

/// Returns whether a DBI storage matches the name `wanted` (for example, "SD Card install").
///
/// DBI numbers its storages: "5: SD Card install". The number is not compared, since it depends on the DBI
/// version, and neither is letter case.
#[must_use]
pub fn storage_matches(storage: &str, wanted: &str) -> bool {
    let name = storage
        .split_once(": ")
        .filter(|(number, _)| number.chars().all(|c| c.is_ascii_digit()))
        .map_or(storage, |(_, name)| name);
    name.trim().eq_ignore_ascii_case(wanted)
}

/// Returns the path to the game logs on the memory card: folders from the root in order.
#[must_use]
pub fn logs_path(title_id: TitleId) -> Vec<String> {
    LOGS_ROOT
        .iter()
        .map(|part| (*part).to_owned())
        .chain([title_id.to_string()])
        .collect()
}

/// Returns whether to fetch a file from the logs folder: the runtime session file (`.shm`, 2 MB) is not a log.
#[must_use]
pub fn is_log_file(name: &str) -> bool {
    !name.to_ascii_lowercase().ends_with(SESSION_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_is_recognised_by_the_nintendo_vendor_id() {
        assert!(is_switch(
            r"\\?\usb#vid_057e&pid_201d#serial#{6ac27878-a6fa-4155-ba85-f98f491d4f33}"
        ));
        assert!(is_switch(r"USB\VID_057E&PID_201D\SERIAL"));
        assert!(!is_switch(r"\\?\usb#vid_04e8&pid_6860#phone#{6ac27878}"));
    }

    #[test]
    fn storages_match_without_their_number() {
        assert!(storage_matches(
            "5: SD Card install",
            InstallTarget::Sd.storage()
        ));
        assert!(storage_matches(
            "6: NAND install",
            InstallTarget::Nand.storage()
        ));
        assert!(storage_matches("1: SD Card", SD_CARD_STORAGE));
        assert!(storage_matches(
            "sd card install",
            InstallTarget::Sd.storage()
        ));
        assert!(!storage_matches("1: SD Card", InstallTarget::Sd.storage()));
        assert!(!storage_matches("7: Saves", SD_CARD_STORAGE));
    }

    #[test]
    fn logs_live_under_the_title_id() {
        let id = TitleId::parse("056694dd13640000").unwrap();
        assert_eq!(logs_path(id), ["switch", "carafe", "056694dd13640000"]);
    }

    #[test]
    fn session_files_are_not_logs() {
        assert!(is_log_file("carafe-overlay.1.log"));
        assert!(is_log_file("autorun_runtime.log"));
        assert!(!is_log_file("wine-nx-session-4294967295.shm"));
    }
}
