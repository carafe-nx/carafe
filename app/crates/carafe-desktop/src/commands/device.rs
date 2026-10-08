//! Switch over MTP.

use carafe_core::ports::DeviceStatus;
use tauri::State;

use crate::error::CommandError;
use crate::services::Services;

/// Returns whether the Switch is visible.
#[tauri::command]
pub async fn device_status(services: State<'_, Services>) -> Result<DeviceStatus, CommandError> {
    Ok(services.device.status())
}
