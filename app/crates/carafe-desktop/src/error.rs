//! A command error in the form the window receives it.

use carafe_core::ports::AdapterError;
use carafe_core::record::RecordError;
use carafe_core::wizard::PlanError;
use serde::Serialize;
use ts_rs::TS;

/// Kind of error; the window uses it to pick the text from the translations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorCode {
    /// A file, folder or game was not found.
    NotFound,
    /// A read or write error.
    Io,
    /// A packing error.
    Pack,
    /// An antivirus blocked the build files.
    Blocked,
    /// The library disk lacks space for the build.
    NoSpace,
    /// A file path during packing is longer than hacBrewPack can read.
    PathTooLong,
    /// The runtime files are missing, unreadable or modified.
    RuntimeDamaged,
    /// The Switch is not connected or failed.
    Device,
    /// An online service is unavailable.
    Network,
    /// The API key is not set or was rejected.
    ApiKey,
    /// The file is in the wrong format.
    Unsupported,
    /// Invalid data from the window.
    Invalid,
    /// A build or a USB install is running and must finish first.
    Busy,
}

/// A command error: a code for translation and a text for the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandError {
    /// Kind of error.
    pub code: ErrorCode,
    /// Details for the log.
    pub message: String,
}

impl CommandError {
    /// Creates an invalid-data error.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Invalid,
            message: message.into(),
        }
    }
}

impl From<AdapterError> for CommandError {
    fn from(error: AdapterError) -> Self {
        let code = match error {
            AdapterError::NotFound(_) => ErrorCode::NotFound,
            AdapterError::Io(_) => ErrorCode::Io,
            AdapterError::Tool(_) => ErrorCode::Pack,
            AdapterError::Blocked(_) => ErrorCode::Blocked,
            AdapterError::NoSpace(_) => ErrorCode::NoSpace,
            AdapterError::PathTooLong(_) => ErrorCode::PathTooLong,
            AdapterError::RuntimeDamaged(_) => ErrorCode::RuntimeDamaged,
            AdapterError::Device(_) => ErrorCode::Device,
            AdapterError::Network(_) => ErrorCode::Network,
            AdapterError::Unauthorized(_) => ErrorCode::ApiKey,
            AdapterError::Unsupported(_) => ErrorCode::Unsupported,
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

impl From<PlanError> for CommandError {
    fn from(error: PlanError) -> Self {
        Self::invalid(error.to_string())
    }
}

impl From<RecordError> for CommandError {
    fn from(error: RecordError) -> Self {
        Self::invalid(error.to_string())
    }
}
