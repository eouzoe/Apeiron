//! Minimal HTTP client over a Unix domain socket.
//!
//! Firecracker exposes its management API via a Unix socket, not TCP.
//! Standard HTTP clients (reqwest) don't support Unix sockets, so we
//! build a thin wrapper using hyper + tokio's `UnixStream`.

use std::path::Path;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, Response, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;

use crate::ExecutorError;

/// Send an HTTP request to a Firecracker API socket.
///
/// The `uri_path` should be the path component only (e.g. `/boot-source`).
/// The host header is set to `localhost` as required by Firecracker.
///
/// # Errors
/// Returns [`ExecutorError::ApiError`] on HTTP or connection errors.
pub async fn api_request(
    socket_path: &Path,
    method: Method,
    uri_path: &str,
    body: Option<String>,
) -> Result<String, ExecutorError> {
    let stream = UnixStream::connect(socket_path).await.map_err(|e| {
        ExecutorError::ApiError(format!("connect to {}: {e}", socket_path.display()))
    })?;

    let io = TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|e| ExecutorError::ApiError(format!("HTTP handshake: {e}")))?;

    // Drive the connection in the background.
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::debug!("Firecracker connection closed: {e}");
        }
    });

    let body_bytes = body.map(Bytes::from).unwrap_or_default();
    let content_len = body_bytes.len();

    let uri: Uri = uri_path
        .parse()
        .map_err(|e| ExecutorError::ApiError(format!("invalid URI path {uri_path}: {e}")))?;

    let mut builder = Request::builder().method(method).uri(uri).header("Host", "localhost");

    if content_len > 0 {
        builder = builder.header("Content-Type", "application/json");
        builder = builder.header("Content-Length", content_len.to_string());
    }

    let req = builder
        .body(Full::new(body_bytes))
        .map_err(|e| ExecutorError::ApiError(format!("build request: {e}")))?;

    let resp: Response<_> = sender
        .send_request(req)
        .await
        .map_err(|e| ExecutorError::ApiError(format!("send request: {e}")))?;

    let status = resp.status();
    let body_bytes = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| ExecutorError::ApiError(format!("read response body: {e}")))?
        .to_bytes();

    let body_str = String::from_utf8_lossy(&body_bytes).into_owned();

    if !status.is_success() {
        return Err(ExecutorError::ApiError(format!("HTTP {status} from {uri_path}: {body_str}")));
    }

    Ok(body_str)
}
