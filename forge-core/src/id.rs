use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a block in the Forge registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BlockId(pub Uuid);

impl BlockId {
    /// Creates a new random `BlockId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner `Uuid`.
    #[must_use]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for BlockId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for BlockId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Unique identifier for a single execution run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    /// Creates a new random `ExecutionId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ExecutionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Identifies a user of the Forge system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UserId(pub String);

impl UserId {
    /// Creates a `UserId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for UserId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Identifies a block contributor in the Forge registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ContributorId(pub String);

impl ContributorId {
    /// Creates a `ContributorId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for ContributorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ContributorId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Identifies a VM snapshot for resumable execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SnapshotId(pub Uuid);

impl SnapshotId {
    /// Creates a new random `SnapshotId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A Nix store path hash identifying a reproducible derivation.
///
/// Format: 32-character base32 Nix hash (e.g. `ywi5ib7yrjba3k3b26yfnbx7gappr3dg`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DerivationHash(pub String);

impl DerivationHash {
    /// Creates a `DerivationHash` from any string-like value.
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }
}

impl fmt::Display for DerivationHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A SHA-256 content hash for verifying deterministic outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    /// Creates a `ContentHash` from a raw 32-byte array.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
