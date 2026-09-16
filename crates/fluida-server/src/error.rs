use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::{
    fmt::{Display, Formatter},
    io,
};

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "VALIDATION_ERROR", message)
    }
    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "INVALID_PATH", message)
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Resource not found")
    }
}
impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ApiError {}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let public = match self.status {
            StatusCode::BAD_REQUEST => "Invalid request",
            StatusCode::UNAUTHORIZED => "Authentication required",
            StatusCode::FORBIDDEN => "Access denied",
            StatusCode::NOT_FOUND => "Resource not found",
            StatusCode::CONFLICT => "Resource conflict",
            StatusCode::PAYLOAD_TOO_LARGE => "Request too large",
            _ => "Unexpected server error",
        };
        (
            self.status,
            Json(json!({"success":false,"error":{"code":self.code,"message":public}})),
        )
            .into_response()
    }
}
impl From<io::Error> for ApiError {
    fn from(value: io::Error) -> Self {
        if value.kind() == io::ErrorKind::NotFound {
            Self::not_found()
        } else {
            Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                value.to_string(),
            )
        }
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        Self::validation(value.to_string())
    }
}
