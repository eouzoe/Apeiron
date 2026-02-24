//! Firecracker microVM lifecycle management for the Forge execution fabric.
//!
//! Handles VM creation, snapshot management, and deterministic task execution
//! within isolated microVM environments.
//!
//! See `docs/ARCHITECTURE.md` for design rationale.

pub mod backend;
pub mod config;
pub mod error;
pub mod firecracker;
pub mod handle;
pub mod orchestrator;
pub mod runner;
pub(crate) mod unix_client;

pub use backend::{ExecutionOutput, VmmBackend};
pub use config::{SnapshotId, VmConfig};
pub use error::ExecutorError;
pub use firecracker::FirecrackerBackend;
pub use handle::VmHandle;
pub use orchestrator::VmOrchestrator;
pub use runner::{compute_hash, BlockRunner};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{ExecutorError, SnapshotId, VmConfig};

    #[test]
    fn vm_config_default_values_are_sane() {
        let config =
            VmConfig::new(PathBuf::from("/tmp/vmlinux"), PathBuf::from("/tmp/rootfs.ext4"));
        assert_eq!(config.vcpu_count, 1, "default vcpu_count should be 1");
        assert_eq!(config.mem_size_mib, 128, "default mem_size_mib should be 128");
        assert!(
            config.boot_args.contains("console=ttyS0"),
            "boot_args should include console=ttyS0"
        );
    }

    #[test]
    fn executor_error_display_includes_context() {
        let err =
            ExecutorError::BinaryNotFound { path: PathBuf::from("/usr/local/bin/firecracker") };
        let msg = err.to_string();
        assert!(msg.contains("firecracker"), "error message should mention the binary name");
        assert!(msg.contains("/usr/local/bin"), "error message should include the path");

        let err2 = ExecutorError::KvmUnavailable { reason: "permission denied".to_owned() };
        let msg2 = err2.to_string();
        assert!(msg2.contains("permission denied"), "KVM error should include the reason");
    }

    #[tokio::test]
    async fn health_check_fails_without_kvm() {
        use crate::{FirecrackerBackend, VmmBackend};

        // Use a non-existent binary path to force BinaryNotFound
        let backend = FirecrackerBackend::new(
            PathBuf::from("/nonexistent/firecracker"),
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        );

        // health_check should fail because the binary doesn't exist
        // (KVM may or may not be available, but binary check comes after)
        let result = backend.health_check().await;

        // If KVM is unavailable, we get KvmUnavailable; otherwise BinaryNotFound
        match result {
            Err(ExecutorError::KvmUnavailable { .. })
            | Err(ExecutorError::BinaryNotFound { .. }) => {
                // expected
            }
            Ok(()) => panic!("health_check should fail with nonexistent binary"),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn snapshot_id_display_is_uuid_format() {
        let id = SnapshotId::new();
        let s = id.to_string();
        // UUID format: 8-4-4-4-12 hex chars
        assert_eq!(s.len(), 36, "UUID string should be 36 chars");
        assert_eq!(s.chars().filter(|&c| c == '-').count(), 4, "UUID should have 4 dashes");
    }
}
