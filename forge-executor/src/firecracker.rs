//! Firecracker VMM backend implementation.
//!
//! Manages Firecracker microVM processes via the Firecracker Management API
//! (HTTP over Unix domain socket).
//!
//! # API Reference
//! Firecracker API spec: `firecracker/src/api_server/swagger/firecracker.yaml`

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use hyper::Method;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use uuid::Uuid;

use crate::backend::{ExecutionOutput, VmmBackend};
use crate::unix_client::api_request;
use crate::{ExecutorError, SnapshotId, VmConfig, VmHandle};

/// Firecracker VMM backend.
///
/// Spawns and manages Firecracker microVM processes, communicating
/// with each via its Unix socket management API.
#[derive(Debug, Clone)]
pub struct FirecrackerBackend {
    /// Path to the `firecracker` binary.
    binary_path: PathBuf,

    /// Directory where per-VM Unix sockets are created.
    socket_dir: PathBuf,

    /// Directory where snapshot files are stored.
    snapshot_dir: PathBuf,
}

impl FirecrackerBackend {
    /// Create a new backend with the given paths.
    ///
    /// # Arguments
    /// - `binary_path`: path to the `firecracker` binary
    /// - `socket_dir`: directory for Unix socket files (must be writable)
    /// - `snapshot_dir`: directory for snapshot state files (must be writable)
    #[must_use]
    pub const fn new(binary_path: PathBuf, socket_dir: PathBuf, snapshot_dir: PathBuf) -> Self {
        Self { binary_path, socket_dir, snapshot_dir }
    }

    /// Create a backend using system defaults.
    ///
    /// Looks for `firecracker` in `$PATH`, uses `/tmp/forge-sockets` and
    /// `/tmp/forge-snapshots` for runtime directories.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(
            PathBuf::from("firecracker"),
            PathBuf::from("/tmp/forge-sockets"),
            PathBuf::from("/tmp/forge-snapshots"),
        )
    }

    fn socket_path(&self, vm_id: Uuid) -> PathBuf {
        self.socket_dir.join(format!("{vm_id}.sock"))
    }

    fn snapshot_mem_path(&self, snapshot_id: SnapshotId) -> PathBuf {
        self.snapshot_dir.join(format!("{snapshot_id}.mem"))
    }

    fn snapshot_state_path(&self, snapshot_id: SnapshotId) -> PathBuf {
        self.snapshot_dir.join(format!("{snapshot_id}.state"))
    }

    /// Wait for the Firecracker API socket to become available.
    async fn wait_for_socket(socket_path: &Path) -> Result<(), ExecutorError> {
        for _ in 0..50u8 {
            if socket_path.exists() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(ExecutorError::SpawnFailed(format!(
            "socket {} did not appear within 5s",
            socket_path.display()
        )))
    }

    /// Configure the VM via the Firecracker API and boot it.
    async fn configure_and_boot(
        socket_path: &Path,
        config: &VmConfig,
    ) -> Result<(), ExecutorError> {
        // Set kernel
        let kernel_body = serde_json::json!({
            "kernel_image_path": config.kernel_path,
            "boot_args": config.boot_args,
        });
        api_request(socket_path, Method::PUT, "/boot-source", Some(kernel_body.to_string()))
            .await?;

        // Set rootfs
        let rootfs_body = serde_json::json!({
            "drive_id": "rootfs",
            "path_on_host": config.rootfs_path,
            "is_root_device": true,
            "is_read_only": false,
        });
        api_request(socket_path, Method::PUT, "/drives/rootfs", Some(rootfs_body.to_string()))
            .await?;

        // Set machine config
        let machine_body = serde_json::json!({
            "vcpu_count": config.vcpu_count,
            "mem_size_mib": config.mem_size_mib,
        });
        api_request(socket_path, Method::PUT, "/machine-config", Some(machine_body.to_string()))
            .await?;

        // Boot
        let boot_body = serde_json::json!({ "action_type": "InstanceStart" });
        api_request(socket_path, Method::PUT, "/actions", Some(boot_body.to_string())).await?;

        Ok(())
    }
}

#[async_trait]
impl VmmBackend for FirecrackerBackend {
    async fn spawn(&self, config: &VmConfig) -> Result<VmHandle, ExecutorError> {
        // Verify KVM is accessible
        if !Path::new("/dev/kvm").exists() {
            return Err(ExecutorError::KvmUnavailable { reason: "/dev/kvm not found".to_owned() });
        }

        // Verify binary exists
        if !self.binary_path.exists() {
            // Try PATH lookup
            which_binary(&self.binary_path)?;
        }

        let vm_id = Uuid::new_v4();
        let socket_path = self.socket_path(vm_id);

        // Ensure socket directory exists
        tokio::fs::create_dir_all(&self.socket_dir).await?;

        tracing::info!(vm_id = %vm_id, socket = %socket_path.display(), "spawning Firecracker VM");

        let process = Command::new(&self.binary_path)
            .arg("--api-sock")
            .arg(&socket_path)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ExecutorError::SpawnFailed(format!("exec firecracker: {e}")))?;

        // Wait for socket
        Self::wait_for_socket(&socket_path).await?;

        // Configure and boot
        Self::configure_and_boot(&socket_path, config)
            .await
            .map_err(|e| ExecutorError::SpawnFailed(e.to_string()))?;

        tracing::info!(vm_id = %vm_id, "VM booted successfully");

        Ok(VmHandle::new(vm_id, socket_path, process))
    }

    async fn snapshot(&self, handle: &VmHandle) -> Result<SnapshotId, ExecutorError> {
        let snapshot_id = SnapshotId::new();

        tokio::fs::create_dir_all(&self.snapshot_dir).await?;

        let mem_path = self.snapshot_mem_path(snapshot_id);
        let state_path = self.snapshot_state_path(snapshot_id);

        tracing::info!(
            vm_id = %handle.id,
            snapshot_id = %snapshot_id,
            "creating VM snapshot"
        );

        // Firecracker requires the VM to be paused before snapshotting.
        let pause_body = serde_json::json!({ "state": "Paused" });
        api_request(&handle.socket_path, Method::PATCH, "/vm", Some(pause_body.to_string()))
            .await
            .map_err(|e| ExecutorError::SnapshotFailed {
                vm_id: handle.id,
                reason: format!("pause failed: {e}"),
            })?;

        let body = serde_json::json!({
            "snapshot_type": "Full",
            "snapshot_path": state_path,
            "mem_file_path": mem_path,
        });

        let result = api_request(
            &handle.socket_path,
            Method::PUT,
            "/snapshot/create",
            Some(body.to_string()),
        )
        .await;

        // Always attempt to resume, even if snapshot failed.
        let resume_body = serde_json::json!({ "state": "Resumed" });
        let _ =
            api_request(&handle.socket_path, Method::PATCH, "/vm", Some(resume_body.to_string()))
                .await;

        result.map_err(|e| ExecutorError::SnapshotFailed {
            vm_id: handle.id,
            reason: e.to_string(),
        })?;

        tracing::info!(snapshot_id = %snapshot_id, "snapshot created");

        Ok(snapshot_id)
    }

    async fn restore(&self, snapshot_id: &SnapshotId) -> Result<VmHandle, ExecutorError> {
        let mem_path = self.snapshot_mem_path(*snapshot_id);
        let state_path = self.snapshot_state_path(*snapshot_id);

        if !mem_path.exists() || !state_path.exists() {
            return Err(ExecutorError::RestoreFailed {
                snapshot_id: snapshot_id.0,
                reason: format!("snapshot files not found at {}", mem_path.display()),
            });
        }

        let vm_id = Uuid::new_v4();
        let socket_path = self.socket_path(vm_id);

        tokio::fs::create_dir_all(&self.socket_dir).await?;

        tracing::info!(
            snapshot_id = %snapshot_id,
            vm_id = %vm_id,
            "restoring VM from snapshot"
        );

        let process = Command::new(&self.binary_path)
            .arg("--api-sock")
            .arg(&socket_path)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ExecutorError::RestoreFailed {
                snapshot_id: snapshot_id.0,
                reason: format!("exec firecracker: {e}"),
            })?;

        Self::wait_for_socket(&socket_path).await.map_err(|e| ExecutorError::RestoreFailed {
            snapshot_id: snapshot_id.0,
            reason: e.to_string(),
        })?;

        let body = serde_json::json!({
            "snapshot_path": state_path,
            "mem_backend": {
                "backend_path": mem_path,
                "backend_type": "File",
            },
            "enable_diff_snapshots": false,
            "resume_vm": true,
        });

        api_request(&socket_path, Method::PUT, "/snapshot/load", Some(body.to_string()))
            .await
            .map_err(|e| ExecutorError::RestoreFailed {
                snapshot_id: snapshot_id.0,
                reason: e.to_string(),
            })?;

        tracing::info!(vm_id = %vm_id, "VM restored from snapshot");

        Ok(VmHandle::new(vm_id, socket_path, process))
    }

    async fn terminate(&self, mut handle: VmHandle) -> Result<(), ExecutorError> {
        tracing::info!(vm_id = %handle.id, "terminating VM");

        handle.process.kill().await?;
        let _ = tokio::fs::remove_file(&handle.socket_path).await;

        tracing::info!(vm_id = %handle.id, "VM terminated");

        Ok(())
    }

    async fn health_check(&self) -> Result<(), ExecutorError> {
        // Check KVM
        if !Path::new("/dev/kvm").exists() {
            return Err(ExecutorError::KvmUnavailable { reason: "/dev/kvm not found".to_owned() });
        }

        // Check KVM is readable
        tokio::fs::metadata("/dev/kvm").await.map_err(|_| ExecutorError::KvmUnavailable {
            reason: "cannot access /dev/kvm (permission denied?)".to_owned(),
        })?;

        // Check binary
        which_binary(&self.binary_path)?;

        Ok(())
    }

    async fn execute_command(
        &self,
        config: &VmConfig,
        command: &str,
        timeout: Duration,
    ) -> Result<ExecutionOutput, ExecutorError> {
        // Verify KVM and binary are available.
        if !Path::new("/dev/kvm").exists() {
            return Err(ExecutorError::KvmUnavailable { reason: "/dev/kvm not found".to_owned() });
        }
        which_binary(&self.binary_path)?;

        let vm_id = Uuid::new_v4();
        let socket_path = self.socket_path(vm_id);
        tokio::fs::create_dir_all(&self.socket_dir).await?;

        // Embed the command as the init process.
        // Separate stdout/stderr via temp files; base64-encode both to survive
        // the serial console's text transport without corruption.
        let init_script = format!(
            "SF=$(mktemp);EF=$(mktemp);eval \"{command}\" >\"$SF\" 2>\"$EF\";EC=$?;\
             echo FORGE_STDOUT_B64_START;base64 \"$SF\";echo FORGE_STDOUT_B64_END;\
             echo FORGE_STDERR_B64_START;base64 \"$EF\";echo FORGE_STDERR_B64_END;\
             echo FORGE_EXIT:$EC;poweroff -f 2>/dev/null||reboot -f"
        );
        let boot_args = format!(
            "console=ttyS0 reboot=k panic=1 pci=off quiet init=/bin/sh -c \"{init_script}\""
        );

        let mut exec_config = config.clone();
        exec_config.boot_args = boot_args;

        tracing::info!(vm_id = %vm_id, %command, "executing command in microVM");

        // Spawn Firecracker with stdout piped so we can read serial console output.
        let mut process = Command::new(&self.binary_path)
            .arg("--api-sock")
            .arg(&socket_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ExecutorError::SpawnFailed(format!("exec firecracker: {e}")))?;

        // Wait for socket, then configure and boot.
        Self::wait_for_socket(&socket_path).await?;
        Self::configure_and_boot(&socket_path, &exec_config)
            .await
            .map_err(|e| ExecutorError::SpawnFailed(e.to_string()))?;

        // Read stdout while waiting for the VM to exit (with timeout).
        let stdout_handle = process
            .stdout
            .take()
            .ok_or_else(|| ExecutorError::SpawnFailed("stdout not piped".to_owned()))?;

        let read_future = async {
            let mut buf = Vec::new();
            let mut reader = tokio::io::BufReader::new(stdout_handle);
            reader.read_to_end(&mut buf).await.map(|_| buf)
        };

        let raw_output = tokio::time::timeout(timeout, read_future)
            .await
            .map_err(|_| {
                ExecutorError::SpawnFailed(format!(
                    "VM did not complete within {}s",
                    timeout.as_secs()
                ))
            })?
            .map_err(ExecutorError::Io)?;

        // Wait for process to fully exit.
        let _ = process.wait().await;
        let _ = tokio::fs::remove_file(&socket_path).await;

        tracing::info!(vm_id = %vm_id, bytes = raw_output.len(), "VM execution complete");

        // Extract stdout, stderr, and exit code from the serial stream.
        let (stdout, stderr, exit_code) = parse_execution_output(&raw_output);

        Ok(ExecutionOutput { stdout, stderr, exit_code })
    }
}

/// Verify a binary exists either at the given path or in PATH.
fn which_binary(path: &Path) -> Result<(), ExecutorError> {
    if path.is_absolute() {
        if path.exists() {
            return Ok(());
        }
        return Err(ExecutorError::BinaryNotFound { path: path.to_owned() });
    }

    // Relative or bare name — check PATH
    let found = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|dir| Path::new(dir).join(path))
        .any(|p| p.exists());

    if found {
        Ok(())
    } else {
        Err(ExecutorError::BinaryNotFound { path: path.to_owned() })
    }
}

/// Parse stdout, stderr, and exit code from raw serial console output.
///
/// Expects the output to contain base64-encoded sections delimited by:
/// - `FORGE_STDOUT_B64_START` / `FORGE_STDOUT_B64_END`
/// - `FORGE_STDERR_B64_START` / `FORGE_STDERR_B64_END`
/// - `FORGE_EXIT:<code>`
///
/// # Returns
/// `(stdout, stderr, exit_code)`. Falls back to `(raw, [], -1)` when markers
/// are absent (e.g. rootfs does not support the init script pattern).
fn parse_execution_output(raw: &[u8]) -> (Vec<u8>, Vec<u8>, i32) {
    let stdout = extract_b64_section(raw, b"FORGE_STDOUT_B64_START", b"FORGE_STDOUT_B64_END");
    let stderr = extract_b64_section(raw, b"FORGE_STDERR_B64_START", b"FORGE_STDERR_B64_END");
    let exit_code = extract_exit_code(raw);

    match (stdout, stderr, exit_code) {
        (Some(out), Some(err), Some(code)) => (out, err, code),
        _ => (raw.to_vec(), Vec::new(), -1),
    }
}

/// Extract and base64-decode a section delimited by `start_marker` / `end_marker`.
///
/// Markers are expected to be followed by `\r\n` (serial console line endings).
/// Returns `None` if either marker is absent or the base64 payload is invalid.
fn extract_b64_section(raw: &[u8], start_marker: &[u8], end_marker: &[u8]) -> Option<Vec<u8>> {
    let start_with_crlf: Vec<u8> = [start_marker, b"\r\n"].concat();

    let content_start = raw
        .windows(start_with_crlf.len())
        .position(|w| w == start_with_crlf.as_slice())
        .map(|p| p + start_with_crlf.len())?;

    let content_end = raw[content_start..]
        .windows(end_marker.len())
        .position(|w| w == end_marker)
        .map(|p| p + content_start)?;

    // Strip \r and \n before decoding — base64 lines are split by the shell.
    let b64_clean: Vec<u8> = raw[content_start..content_end]
        .iter()
        .copied()
        .filter(|&b| b != b'\r' && b != b'\n')
        .collect();

    base64::engine::general_purpose::STANDARD.decode(&b64_clean).ok()
}

/// Extract the integer exit code from a `FORGE_EXIT:<N>` line.
///
/// Returns `None` if the marker is absent or the value cannot be parsed.
fn extract_exit_code(raw: &[u8]) -> Option<i32> {
    let marker = b"FORGE_EXIT:";
    let value_start =
        raw.windows(marker.len()).position(|w| w == marker).map(|p| p + marker.len())?;

    let rest = &raw[value_start..];
    let value_end = rest.iter().position(|&b| b == b'\r' || b == b'\n').unwrap_or(rest.len());

    std::str::from_utf8(&rest[..value_end]).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_b64_output(stdout: &[u8], stderr: &[u8], exit_code: i32) -> Vec<u8> {
        use base64::Engine as _;
        let stdout_b64 = base64::engine::general_purpose::STANDARD.encode(stdout);
        let stderr_b64 = base64::engine::general_purpose::STANDARD.encode(stderr);
        format!(
            "kernel boot noise\r\nFORGE_STDOUT_B64_START\r\n{stdout_b64}\r\nFORGE_STDOUT_B64_END\r\n\
             FORGE_STDERR_B64_START\r\n{stderr_b64}\r\nFORGE_STDERR_B64_END\r\n\
             FORGE_EXIT:{exit_code}\r\n"
        )
        .into_bytes()
    }

    #[test]
    fn parse_execution_output_extracts_stdout_and_stderr() {
        let raw = make_b64_output(b"hello stdout\n", b"hello stderr\n", 0);
        let (stdout, stderr, _) = parse_execution_output(&raw);
        assert_eq!(stdout, b"hello stdout\n");
        assert_eq!(stderr, b"hello stderr\n");
    }

    #[test]
    fn parse_execution_output_decodes_base64() {
        let binary_payload: Vec<u8> = (0u8..=255).collect();
        let raw = make_b64_output(&binary_payload, b"", 0);
        let (stdout, _, _) = parse_execution_output(&raw);
        assert_eq!(stdout, binary_payload, "binary payload must survive base64 round-trip");
    }

    #[test]
    fn parse_execution_output_fallback_on_missing_markers() {
        let raw = b"raw serial output without any markers";
        let (stdout, stderr, exit_code) = parse_execution_output(raw);
        assert_eq!(stdout, raw, "fallback stdout must equal raw input");
        assert!(stderr.is_empty(), "fallback stderr must be empty");
        assert_eq!(exit_code, -1, "fallback exit code must be -1");
    }

    #[test]
    fn parse_execution_output_extracts_exit_code() {
        let raw = make_b64_output(b"out", b"err", 42);
        let (_, _, exit_code) = parse_execution_output(&raw);
        assert_eq!(exit_code, 42, "exit code must be extracted from FORGE_EXIT marker");
    }

    #[test]
    fn extract_exit_code_negative_value_is_parsed() {
        let raw = b"FORGE_EXIT:-1\r\n";
        let code = extract_exit_code(raw);
        assert_eq!(code, Some(-1), "negative exit code must be parsed correctly");
    }

    #[test]
    fn extract_exit_code_missing_marker_returns_none() {
        let raw = b"no marker here";
        assert_eq!(extract_exit_code(raw), None, "missing marker must return None");
    }

    #[test]
    fn extract_b64_section_missing_start_returns_none() {
        let raw = b"FORGE_STDOUT_B64_END\r\n";
        let result = extract_b64_section(raw, b"FORGE_STDOUT_B64_START", b"FORGE_STDOUT_B64_END");
        assert!(result.is_none(), "missing start marker must return None");
    }

    #[test]
    fn parse_execution_output_empty_stdout_and_stderr() {
        let raw = make_b64_output(b"", b"", 0);
        let (stdout, stderr, exit_code) = parse_execution_output(&raw);
        assert!(stdout.is_empty(), "empty stdout must decode to empty vec");
        assert!(stderr.is_empty(), "empty stderr must decode to empty vec");
        assert_eq!(exit_code, 0);
    }

    proptest::proptest! {
        #[test]
        fn proptest_exit_code_roundtrip(code in proptest::prelude::any::<i32>()) {
            let raw = format!("FORGE_EXIT:{code}\r\n").into_bytes();
            let extracted = extract_exit_code(&raw);
            proptest::prop_assert_eq!(extracted, Some(code));
        }

        #[test]
        fn proptest_base64_roundtrip(
            data in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512usize)
        ) {
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&encoded)
                .map_err(|e| proptest::test_runner::TestCaseError::fail(
                    format!("base64 decode of freshly encoded data failed: {e}")
                ))?;
            proptest::prop_assert_eq!(decoded, data, "base64 encode→decode must be identity");
        }
    }
}
