//! Response types and helpers for HTTP endpoints.

use serde::Serialize;

/// Consistent API response wrapper for success responses
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// Always true for success responses
    pub success: bool,
    /// Response data
    pub data: T,
}

/// Consistent API error response wrapper
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// Error code (HTTP status code as string)
    pub code: String,
    /// Error message
    pub message: String,
    /// Optional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Consistent error response wrapper
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Always false for error responses
    pub success: bool,
    /// Error information
    pub error: ApiError,
}

/// Helper to create success response
pub fn success_response<T: Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse {
        success: true,
        data,
    }
}

/// Helper to create error response
pub fn error_response(code: u16, message: String, details: Option<String>) -> ErrorResponse {
    ErrorResponse {
        success: false,
        error: ApiError {
            code: code.to_string(),
            message,
            details,
        },
    }
}
