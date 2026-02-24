//! Core types for the Forge deterministic execution fabric.
//!
//! Defines the fundamental domain types: blocks, execution records,
//! trust scores, and the dependency/capability model.
//!
//! See `docs/ARCHITECTURE.md` for design rationale.

/// Block definition and manifest types.
pub mod block;
/// Error types for the core crate.
pub mod error;
/// Example blocks for testing and documentation.
pub mod examples;
/// Execution record and status types.
pub mod execution;
/// Identifier types (`BlockId`, `ContentHash`, etc.).
pub mod id;
/// Trust score and level types.
pub mod trust;

pub use block::{Block, BlockManifest, Capability, CognitiveLoad, Dependency, DependencyKind};
pub use error::CoreError;
pub use execution::{ExecutionRecord, ExecutionStatus};
pub use id::{
    BlockId, ContentHash, ContributorId, DerivationHash, ExecutionId, SnapshotId, UserId,
};
pub use trust::{SemVer, TrustLevel, TrustScore};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::examples::example_blocks;

    #[test]
    fn trust_score_valid_range_accepts() {
        assert!(TrustScore::new(0.0).is_ok());
        assert!(TrustScore::new(0.5).is_ok());
        assert!(TrustScore::new(1.0).is_ok());
    }

    #[test]
    fn trust_score_out_of_range_rejects() {
        assert!(TrustScore::new(-0.1).is_err());
        assert!(TrustScore::new(1.1).is_err());
        assert!(TrustScore::new(f64::NAN).is_err());
        assert!(TrustScore::new(f64::INFINITY).is_err());
    }

    #[test]
    fn content_hash_display_shows_hex() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0xde;
        bytes[1] = 0xad;
        bytes[31] = 0xff;
        let hash = ContentHash::new(bytes);
        let s = hash.to_string();
        assert!(s.starts_with("dead"), "expected hex starting with 'dead', got {s}");
        assert!(s.ends_with("ff"), "expected hex ending with 'ff', got {s}");
        assert_eq!(s.len(), 64, "SHA-256 hex must be 64 chars");
    }

    #[test]
    fn example_blocks_all_valid() {
        let blocks = example_blocks();
        assert_eq!(blocks.len(), 3);

        let git = &blocks[0];
        assert_eq!(git.manifest.name, "git-env");
        assert_eq!(git.manifest.minimum_trust_level, TrustLevel::Zero);
        assert_eq!(git.manifest.cognitive_load, CognitiveLoad::Low);
        assert!(git.manifest.requires.is_empty());
        assert_eq!(git.manifest.provides.len(), 1);

        let rust_dev = &blocks[1];
        assert_eq!(rust_dev.manifest.name, "rust-dev-env");
        assert_eq!(rust_dev.manifest.minimum_trust_level, TrustLevel::One);
        assert_eq!(rust_dev.manifest.cognitive_load, CognitiveLoad::Medium);
        assert_eq!(rust_dev.manifest.requires.len(), 1);

        let bose = &blocks[2];
        assert_eq!(bose.manifest.name, "bose-search");
        assert_eq!(bose.manifest.minimum_trust_level, TrustLevel::Two);
        assert_eq!(bose.manifest.cognitive_load, CognitiveLoad::High);
        assert_eq!(bose.manifest.requires.len(), 2);
    }

    #[test]
    fn semver_display_formats_correctly() {
        let v = SemVer::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");

        let v0 = SemVer::new(0, 0, 0);
        assert_eq!(v0.to_string(), "0.0.0");
    }

    #[test]
    fn content_hash_as_bytes_returns_raw_bytes() {
        let bytes = [0xab_u8; 32];
        let hash = ContentHash::new(bytes);
        assert_eq!(hash.as_bytes(), &bytes, "as_bytes must return the original array");
    }

    #[test]
    fn content_hash_equality_same_bytes() {
        let bytes = [0x42_u8; 32];
        let a = ContentHash::new(bytes);
        let b = ContentHash::new(bytes);
        assert_eq!(a, b, "ContentHashes with identical bytes must be equal");
    }

    #[test]
    fn trust_score_value_returns_inner_f64() {
        let score = match TrustScore::new(0.75) {
            Ok(s) => s,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert!((score.value() - 0.75).abs() < f64::EPSILON, "value() must return the inner f64");
    }

    #[test]
    fn trust_score_display_formats_to_4_decimal_places() {
        let score = match TrustScore::new(0.5) {
            Ok(s) => s,
            Err(e) => panic!("unexpected error: {e}"),
        };
        assert_eq!(score.to_string(), "0.5000", "Display must format to 4 decimal places");
    }

    #[test]
    fn trust_score_try_from_valid_value_succeeds() {
        let result = TrustScore::try_from(0.9_f64);
        assert!(result.is_ok(), "TryFrom valid value must succeed");
    }

    #[test]
    fn execution_record_new_sets_correct_fields() {
        use crate::execution::ExecutionStatus;
        use crate::id::{BlockId, ContentHash, UserId};
        use chrono::Utc;
        use std::time::Duration;

        let block_id = BlockId::new();
        let user_id = UserId::new("test-user");
        let input_hash = ContentHash::new([0u8; 32]);
        let output_hash = ContentHash::new([1u8; 32]);
        let started_at = Utc::now();
        let duration = Duration::from_millis(100);

        let record = ExecutionRecord::new(
            block_id,
            user_id.clone(),
            input_hash,
            output_hash,
            started_at,
            duration,
            ExecutionStatus::Succeeded,
        );

        assert_eq!(record.block_id, block_id);
        assert_eq!(record.user_id, user_id);
        assert_eq!(record.input_hash, input_hash);
        assert_eq!(record.output_hash, output_hash);
        assert_eq!(record.duration, duration);
        assert!(record.vm_snapshot_id.is_none(), "vm_snapshot_id must default to None");
    }

    #[test]
    fn execution_status_failed_contains_reason() {
        use crate::execution::ExecutionStatus;
        let status = ExecutionStatus::Failed { reason: "out of memory".to_owned() };
        match status {
            ExecutionStatus::Failed { reason } => {
                assert_eq!(reason, "out of memory");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}
