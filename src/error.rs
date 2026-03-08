use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
    pub hint: Option<String>,
}

impl AppError {
    pub fn bad_request(msg: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.to_string(),
            hint: None,
        }
    }

    pub fn not_found(msg: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.to_string(),
            hint: None,
        }
    }

    pub fn unauthorized(msg: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: msg.to_string(),
            hint: Some("Include your API key: Authorization: Bearer YOUR_API_KEY".to_string()),
        }
    }

    pub fn forbidden(msg: &str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: msg.to_string(),
            hint: None,
        }
    }

    pub fn conflict(msg: &str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.to_string(),
            hint: None,
        }
    }

    pub fn internal(_msg: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Internal server error".to_string(),
            hint: None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            success: false,
            error: self.message,
            hint: self.hint,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<worker::Error> for AppError {
    fn from(_err: worker::Error) -> Self {
        AppError::internal("")
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::bad_request(&format!("Invalid JSON: {}", err))
    }
}
