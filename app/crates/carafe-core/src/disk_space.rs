//! How much disk space an NSP build needs.

use thiserror::Error;

/// How many times the size of the game with the runtime the build takes on disk at its peak.
pub const PEAK_FACTOR: u64 = 4;

/// Reserve on top of the peak, in bytes.
pub const RESERVE_BYTES: u64 = 256 << 20;

/// Free space is less than the build needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("{} needed, {} free", gib(*.required), gib(*.available))]
pub struct NotEnoughSpace {
    /// How many bytes are needed.
    pub required: u64,
    /// How many bytes are free.
    pub available: u64,
}

/// Returns how many bytes of disk space the build needs.
///
/// `content_bytes` is the size of the unpacked runtime and the game files together.
#[must_use]
pub fn required_space(content_bytes: u64) -> u64 {
    content_bytes
        .saturating_mul(PEAK_FACTOR)
        .saturating_add(RESERVE_BYTES)
}

/// Checks whether there is enough disk space for the build.
///
/// # Errors
///
/// [`NotEnoughSpace`] if `available` is less than [`required_space`] of `content_bytes`.
pub fn check_space(content_bytes: u64, available: u64) -> Result<(), NotEnoughSpace> {
    let required = required_space(content_bytes);
    if available < required {
        return Err(NotEnoughSpace {
            required,
            available,
        });
    }
    Ok(())
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / f64::from(1u32 << 30))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn a_build_needs_four_times_its_content_plus_the_reserve() {
        assert_eq!(required_space(GIB), 4 * GIB + RESERVE_BYTES);
    }

    #[test]
    fn exactly_enough_space_passes() {
        assert_eq!(check_space(GIB, 4 * GIB + RESERVE_BYTES), Ok(()));
    }

    #[test]
    fn one_byte_short_is_reported_in_gibibytes() {
        let error = check_space(GIB, 4 * GIB + RESERVE_BYTES - 1).unwrap_err();
        assert_eq!(error.required, 4 * GIB + RESERVE_BYTES);
        assert_eq!(error.to_string(), "4.2 GB needed, 4.2 GB free");
    }

    #[test]
    fn a_huge_content_does_not_overflow() {
        assert_eq!(required_space(u64::MAX), u64::MAX);
    }
}
