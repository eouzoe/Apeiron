use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Semantic version following the `major.minor.patch` scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SemVer {
    /// Major version — incremented on breaking changes.
    pub major: u32,
    /// Minor version — incremented on backwards-compatible additions.
    pub minor: u32,
    /// Patch version — incremented on backwards-compatible bug fixes.
    pub patch: u32,
}

impl SemVer {
    /// Creates a new `SemVer`.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Trust level required to use or compose a block.
///
/// Higher levels unlock more powerful but potentially risky operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TrustLevel {
    /// Level 0 — can only use pre-approved block combinations.
    Zero,
    /// Level 1 — can swap individual blocks within approved compositions.
    One,
    /// Level 2 — can compose blocks from scratch.
    Two,
    /// Level 3 — can contribute new blocks to the registry.
    Three,
}

/// A normalised trust score in the range `[0.0, 1.0]`.
///
/// Computed from execution history, audit results, and contributor reputation.
/// Higher scores indicate more reliable, well-tested blocks.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TrustScore(f64);

impl TrustScore {
    /// Creates a `TrustScore` from a value in `[0.0, 1.0]`.
    ///
    /// # Errors
    /// Returns [`CoreError::InvalidTrustScore`] if `value` is outside `[0.0, 1.0]`.
    pub fn new(value: f64) -> Result<Self, CoreError> {
        if value.is_nan() || !(0.0..=1.0).contains(&value) {
            return Err(CoreError::InvalidTrustScore { value });
        }
        Ok(Self(value))
    }

    /// Returns the inner `f64` value.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for TrustScore {
    type Error = CoreError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for TrustScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}
