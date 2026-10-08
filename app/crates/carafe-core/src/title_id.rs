//! Application Title ID following the Carafe rule.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;
use ts_rs::TS;

/// Switch application identifier following the Carafe rule: `0x05XXXXXXXXXX[02468ace]000`.
///
/// The high byte is `0x05`, the fourth hex digit from the end is even, the three lowest are zeros.
/// As long as the Title ID stays the same, reinstalling over the game keeps its save data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, TS)]
#[ts(export, type = "string")]
pub struct TitleId(u64);

/// Error parsing a [`TitleId`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TitleIdError {
    /// The string is not 16 hex digits (with or without `0x`).
    #[error("Title ID must consist of 16 hex digits: {0}")]
    Malformed(String),
    /// The number does not match the rule `0x05XXXXXXXXXX[02468ace]000`.
    #[error("Title ID {0:016x} does not match the rule 0x05XXXXXXXXXX[02468ace]000")]
    OutOfRule(u64),
}

impl TitleId {
    const PREFIX: u64 = 0x05;
    const MIDDLE_MASK: u64 = 0xFF_FFFF_FFFF;

    /// Creates a Title ID from a number.
    ///
    /// # Errors
    ///
    /// [`TitleIdError::OutOfRule`] if the number does not match the Carafe rule.
    pub fn new(value: u64) -> Result<Self, TitleIdError> {
        let prefix_ok = value >> 56 == Self::PREFIX;
        let even_ok = (value >> 12) & 1 == 0;
        let tail_ok = value & 0xFFF == 0;
        if prefix_ok && even_ok && tail_ok {
            Ok(Self(value))
        } else {
            Err(TitleIdError::OutOfRule(value))
        }
    }

    /// Parses a Title ID from 16 hex digits, with or without the `0x` prefix.
    ///
    /// # Errors
    ///
    /// [`TitleIdError::Malformed`] for a string of any other form, [`TitleIdError::OutOfRule`] for a number
    /// outside the rule.
    pub fn parse(text: &str) -> Result<Self, TitleIdError> {
        let digits = text.strip_prefix("0x").unwrap_or(text);
        let well_formed = digits.len() == 16 && digits.chars().all(|c| c.is_ascii_hexdigit());
        if !well_formed {
            return Err(TitleIdError::Malformed(text.to_owned()));
        }
        let value = u64::from_str_radix(digits, 16)
            .map_err(|_| TitleIdError::Malformed(text.to_owned()))?;
        Self::new(value)
    }

    /// Creates a Title ID following the rule from random bits.
    ///
    /// The lowest 43 bits of `entropy` are used: 40 for the middle, 3 for the even digit.
    #[must_use]
    pub fn from_entropy(entropy: u64) -> Self {
        let middle = entropy & Self::MIDDLE_MASK;
        let even_digit = ((entropy >> 40) & 0b111) << 1;
        Self(Self::PREFIX << 56 | middle << 16 | even_digit << 12)
    }

    /// Returns the numeric value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TitleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl Serialize for TitleId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TitleId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ids_used_on_console() {
        for text in ["055deb23507ec000", "0x056694dd13640000", "055cddb0a2302000"] {
            assert!(TitleId::parse(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn rejects_odd_fourth_digit_and_foreign_prefix() {
        assert_eq!(
            TitleId::parse("0544670f1a32b000"),
            Err(TitleIdError::OutOfRule(0x0544_670f_1a32_b000))
        );
        assert!(matches!(
            TitleId::parse("0100000000010000"),
            Err(TitleIdError::OutOfRule(_))
        ));
    }

    #[test]
    fn rejects_malformed_text() {
        assert!(matches!(
            TitleId::parse("05zz"),
            Err(TitleIdError::Malformed(_))
        ));
        assert!(matches!(
            TitleId::parse("055deb23507ec0000"),
            Err(TitleIdError::Malformed(_))
        ));
    }

    #[test]
    fn entropy_always_gives_valid_id() {
        for entropy in [0, u64::MAX, 0x1234_5678_9abc_def0, 1 << 40, 7 << 40] {
            let id = TitleId::from_entropy(entropy);
            assert_eq!(TitleId::new(id.value()), Ok(id));
        }
    }

    #[test]
    fn round_trips_through_json_as_string() {
        let id = TitleId::parse("055deb23507ec000").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"055deb23507ec000\"");
        assert_eq!(serde_json::from_str::<TitleId>(&json).unwrap(), id);
    }
}
