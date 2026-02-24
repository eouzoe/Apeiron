//! Error types for the gateway crate.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

/// Errors that can occur during gateway request handling.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GatewayError {
    /// An error propagated from the executor layer.
    #[error("executor error: {0}")]
    Executor(#[from] forge_executor::ExecutorError),

    /// The requested sandbox ID does not exist in the pool.
    #[error("sandbox not found: {0}")]
    SandboxNotFound(Uuid),

    /// The request body is malformed or contains invalid values.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Executor(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SandboxNotFound(_) => StatusCode::NOT_FOUND,
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        };
        (status, Json(json!({"error": self.to_string()}))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn gateway_error_status_codes_map_correctly() {
        let not_found = GatewayError::SandboxNotFound(Uuid::nil());
        let resp = not_found.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let bad_req = GatewayError::InvalidRequest("missing field".to_owned());
        let resp = bad_req.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn gateway_error_executor_variant_returns_500() {
        use forge_executor::ExecutorError;
        let exec_err = ExecutorError::SpawnFailed("vm died".to_owned());
        let gw_err = GatewayError::Executor(exec_err);
        let resp = gw_err.into_response();
        assert_eq!(
            resp.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "Executor errors must map to 500"
        );
    }

    #[test]
    fn gateway_error_display_includes_message() {
        let err = GatewayError::InvalidRequest("bad runtime".to_owned());
        let msg = err.to_string();
        assert!(msg.contains("bad runtime"), "Display must include the message");
    }
}
