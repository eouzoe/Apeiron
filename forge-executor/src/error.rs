//! Error types for the executor crate.

use std::path::PathBuf;

use uuid::Uuid;

/// Errors that can occur during VM lifecycle operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExecutorError {
    /// Firecracker binary not found at the configured path.
    #[error("firecracker binary not found at {path}")]
    BinaryNotFound {
        /// The path where the binary was expected.
        path: PathBuf,
    },

    /// KVM device is not available or not accessible.
    #[error("KVM not available: {reason}")]
    KvmUnavailable {
        /// The reason KVM is unavailable.
        reason: String,
    },

    /// VM failed to spawn.
    #[error("VM spawn failed: {0}")]
    SpawnFailed(String),

    /// Snapshot operation failed.
    #[error("snapshot failed for VM {vm_id}: {reason}")]
    SnapshotFailed {
        /// The ID of the VM that failed to snapshot.
        vm_id: Uuid,
        /// The reason the snapshot failed.
        reason: String,
    },

    /// Restore from snapshot failed.
    #[error("restore failed for snapshot {snapshot_id}: {reason}")]
    RestoreFailed {
        /// The ID of the snapshot that failed to restore.
        snapshot_id: Uuid,
        /// The reason the restore failed.
        reason: String,
    },

    /// Firecracker API request failed.
    #[error("API request failed: {0}")]
    ApiError(String),

    /// VM not found in the active registry.
    #[error("VM not found: {0}")]
    VmNotFound(Uuid),

    /// Underlying I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
