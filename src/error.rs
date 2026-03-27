use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use std::fmt;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub status: StatusCode,
}

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>, status: StatusCode) -> Self {
        Self {
            code,
            message: message.into(),
            status,
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new("not_found", msg, StatusCode::NOT_FOUND)
    }

    pub fn remote(msg: impl Into<String>) -> Self {
        Self::new("remote_error", msg, StatusCode::BAD_GATEWAY)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new("internal_error", msg, StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new("unauthorized", msg, StatusCode::UNAUTHORIZED)
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new("bad_request", msg, StatusCode::BAD_REQUEST)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::remote(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: self.code.to_string(),
            message: self.message,
        };
        (self.status, axum::Json(body)).into_response()
    }
}
