//! Release versions of Carafe and its runtime.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

/// A release version `MAJOR.MINOR.PATCH`, as in the release tags without the leading `v`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

/// The text is not a release version.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("'{0}' is not a version like 1.2.3")]
pub struct VersionError(pub String);

impl Version {
    /// Returns the version from text like `1.2.3` or `v1.2.3`.
    ///
    /// # Errors
    ///
    /// [`VersionError`] if the text is not three non-negative whole numbers separated by dots.
    pub fn parse(text: &str) -> Result<Self, VersionError> {
        let error = || VersionError(text.to_owned());
        let bare = text.strip_prefix('v').unwrap_or(text);
        let mut parts = bare.split('.').map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
            let leading_zero = part.len() > 1 && part.starts_with('0');
            if digits && !leading_zero {
                part.parse::<u32>().map_err(|_| error())
            } else {
                Err(error())
            }
        });
        let major = parts.next().ok_or_else(error)??;
        let minor = parts.next().ok_or_else(error)??;
        let patch = parts.next().ok_or_else(error)??;
        if parts.next().is_some() {
            return Err(error());
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(de::Error::custom)
    }
}

/// Returns whether a game built with the runtime `built_with` lacks fixes of the runtime `current`.
///
/// Versions are compared as numbers. If either is not a release version, any difference counts as
/// outdated.
#[must_use]
pub fn runtime_outdated(built_with: &str, current: &str) -> bool {
    match (Version::parse(built_with), Version::parse(current)) {
        (Ok(built_with), Ok(current)) => built_with < current,
        _ => built_with != current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_with_and_without_v() {
        assert_eq!(Version::parse("0.2.10"), Version::parse("v0.2.10"));
        assert_eq!(Version::parse("0.2.10").unwrap().to_string(), "0.2.10");
    }

    #[test]
    fn rejects_anything_but_three_numbers() {
        for text in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "1.2.x",
            "1.02.3",
            "1.2.3-beta",
            " 1.2.3",
            "-1.2.3",
        ] {
            assert!(Version::parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn compares_as_numbers() {
        assert!(Version::parse("0.10.0").unwrap() > Version::parse("0.9.9").unwrap());
        assert!(Version::parse("1.0.0").unwrap() > Version::parse("0.99.99").unwrap());
    }

    #[test]
    fn travels_as_text() {
        let version = Version::parse("1.2.3").unwrap();
        let json = serde_json::to_string(&version).unwrap();
        assert_eq!(json, r#""1.2.3""#);
        assert_eq!(serde_json::from_str::<Version>(&json).unwrap(), version);
        assert!(serde_json::from_str::<Version>(r#""1.2""#).is_err());
    }

    #[test]
    fn older_runtime_is_outdated() {
        assert!(runtime_outdated("0.1.0", "0.1.1"));
        assert!(runtime_outdated("0.9.0", "0.10.0"));
    }

    #[test]
    fn same_or_newer_runtime_is_not_outdated() {
        assert!(!runtime_outdated("0.1.1", "0.1.1"));
        assert!(!runtime_outdated("0.2.0", "0.1.1"));
    }

    #[test]
    fn unknown_versions_differ_or_match() {
        assert!(runtime_outdated("dev", "0.1.1"));
        assert!(!runtime_outdated("dev", "dev"));
    }
}
