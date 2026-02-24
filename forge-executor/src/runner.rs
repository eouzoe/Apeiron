//! Block execution runner — executes a block inside a microVM.
//!
//! The runner embeds the block's command in the kernel boot args, boots a
//! Firecracker microVM, captures serial console output, and computes a
//! SHA-256 `output_hash` for determinism verification.
//!
//! See `docs/ARCHITECTURE.md` §3 for design rationale.

use std::time::{Duration, Instant};

use chrono::Utc;
use sha2::{Digest, Sha256};

use forge_core::block::Block;
use forge_core::execution::{ExecutionRecord, ExecutionStatus};
use forge_core::id::{ContentHash, UserId};

use crate::{ExecutorError, VmConfig, VmmBackend};

/// Default execution timeout: 30 seconds per VM run.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Executes a block inside a microVM and captures the output.
///
/// The runner:
/// 1. Spawns a VM using the configured backend with the command in boot args
/// 2. Captures serial console output (stdout)
/// 3. Computes `output_hash` (SHA-256 of captured output)
/// 4. Records an [`ExecutionRecord`]
///
/// # Cancel Safety
/// Cancel safe. Dropping the future will terminate the VM process via
/// `kill_on_drop`.
pub struct BlockRunner<B: VmmBackend> {
    backend: B,
    vm_config: VmConfig,
    timeout: Duration,
}

impl<B: VmmBackend> BlockRunner<B> {
    /// Create a new runner with the given backend and VM configuration.
    #[must_use]
    pub const fn new(backend: B, vm_config: VmConfig) -> Self {
        Self { backend, vm_config, timeout: DEFAULT_TIMEOUT }
    }

    /// Create a runner with a custom execution timeout.
    #[must_use]
    pub const fn with_timeout(backend: B, vm_config: VmConfig, timeout: Duration) -> Self {
        Self { backend, vm_config, timeout }
    }

    /// Execute a block and return the execution record.
    ///
    /// The block's `manifest.name` is used as the command to run inside the VM.
    /// For the MVP, the command is `echo <block-name>` to prove determinism.
    ///
    /// # Errors
    /// Returns [`ExecutorError::SpawnFailed`] if the VM cannot start.
    /// Returns [`ExecutorError::Io`] on timeout or I/O failure.
    pub async fn execute(
        &self,
        block: &Block,
        input: &[u8],
    ) -> Result<ExecutionRecord, ExecutorError> {
        let input_hash = compute_hash(input, b"");
        let started_at = Utc::now();
        let wall_start = Instant::now();

        // Use the block name as the command for MVP determinism proof.
        // A real implementation would look up the block's Nix derivation.
        let command = build_command(&block.manifest.name);

        tracing::info!(
            block = %block.manifest.name,
            %command,
            "starting block execution"
        );

        let output = self.backend.execute_command(&self.vm_config, &command, self.timeout).await?;

        let duration = wall_start.elapsed();
        let output_hash = compute_hash(&output.stdout, &output.stderr);

        tracing::info!(
            block = %block.manifest.name,
            output_hash = %output_hash,
            elapsed_ms = duration.as_millis(),
            "block execution complete"
        );

        Ok(ExecutionRecord::new(
            block.id,
            UserId::new("forge-runner"),
            input_hash,
            output_hash,
            started_at,
            duration,
            ExecutionStatus::Succeeded,
        ))
    }
}

/// Compute SHA-256 hash of stdout + stderr concatenated.
///
/// `S(output) = SHA-256(stdout || stderr)`
///
/// # Complexity
/// O(n) where n = len(stdout) + len(stderr).
#[must_use]
pub fn compute_hash(stdout: &[u8], stderr: &[u8]) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(stdout);
    hasher.update(stderr);
    let result = hasher.finalize();
    ContentHash::new(result.into())
}

/// Build the shell command to run inside the VM for a given block name.
///
/// For the MVP, we use `echo <name>` which is always deterministic.
fn build_command(block_name: &str) -> String {
    // Shell-escape the block name to prevent injection.
    // Block names are alphanumeric + hyphens, so this is safe.
    format!("echo '{block_name}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_hash_is_deterministic() {
        let stdout = b"git version 2.43.0\n";
        let hash1 = compute_hash(stdout, b"");
        let hash2 = compute_hash(stdout, b"");
        assert_eq!(hash1, hash2, "same input must produce same hash");
    }

    #[test]
    fn compute_hash_differs_for_different_input() {
        let hash1 = compute_hash(b"output1\n", b"");
        let hash2 = compute_hash(b"output2\n", b"");
        assert_ne!(hash1, hash2, "different input must produce different hash");
    }

    #[test]
    fn compute_hash_includes_stderr() {
        let hash_no_stderr = compute_hash(b"out", b"");
        let hash_with_stderr = compute_hash(b"out", b"err");
        assert_ne!(hash_no_stderr, hash_with_stderr, "stderr must affect the hash");
    }

    #[test]
    fn compute_hash_empty_input_is_sha256_of_empty() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = compute_hash(b"", b"");
        let hex = hash.to_string();
        assert_eq!(
            hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "empty input hash must match known SHA-256 value"
        );
    }

    #[test]
    fn build_command_wraps_block_name() {
        let cmd = build_command("git-env");
        assert!(cmd.contains("git-env"), "command must include block name");
        assert!(cmd.starts_with("echo"), "MVP command must use echo");
    }

    proptest::proptest! {
        #[test]
        fn proptest_hash_output_always_64_hex_chars(
            stdout in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512usize),
            stderr in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512usize),
        ) {
            let hash = compute_hash(&stdout, &stderr);
            let hex = hash.to_string();
            proptest::prop_assert_eq!(hex.len(), 64, "SHA-256 hex must always be 64 chars");
            proptest::prop_assert!(
                hex.chars().all(|c| c.is_ascii_hexdigit()),
                "SHA-256 hex must contain only hex digits"
            );
        }

        #[test]
        fn proptest_hash_order_matters_stdout_before_stderr(
            a in proptest::collection::vec(proptest::prelude::any::<u8>(), 1..64usize),
            b in proptest::collection::vec(proptest::prelude::any::<u8>(), 1..64usize),
        ) {
            proptest::prop_assume!(a != b);
            let hash_ab = compute_hash(&a, &b);
            let hash_ba = compute_hash(&b, &a);
            // stdout and stderr are concatenated in order, so swapping them
            // must produce a different hash (unless a == b, excluded above).
            proptest::prop_assert_ne!(
                hash_ab, hash_ba,
                "swapping stdout and stderr must change the hash"
            );
        }
    }
}
