/// Error types for the UPP SDK
use thiserror::Error;

/// Result type for UPP SDK operations
pub type Result<T> = std::result::Result<T, UppSdkError>;

/// Comprehensive error type for the UPP SDK
#[derive(Error, Debug)]
pub enum UppSdkError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    /// Failed to serialize/deserialize JSON
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Invalid URL provided
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    /// API returned an error response
    #[error("API error: status={status}, body={body}")]
    ApiError { status: u16, body: String },

    /// Request validation failed
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Missing required parameter
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    /// Timeout during request
    #[error("Request timeout")]
    Timeout,

    /// Client not configured properly
    #[error("Client configuration error: {0}")]
    ConfigError(String),

    /// Unexpected response format
    #[error("Unexpected response format: {0}")]
    UnexpectedResponse(String),
}

impl UppSdkError {
    /// Create a validation error
    pub fn validation(msg: impl Into<String>) -> Self {
        UppSdkError::ValidationError(msg.into())
    }

    /// Create a missing parameter error
    pub fn missing_param(param: impl Into<String>) -> Self {
        UppSdkError::MissingParameter(param.into())
    }

    /// Create a config error
    pub fn config(msg: impl Into<String>) -> Self {
        UppSdkError::ConfigError(msg.into())
    }

    /// Create an API error
    pub fn api_error(status: u16, body: impl Into<String>) -> Self {
        UppSdkError::ApiError {
            status,
            body: body.into(),
        }
    }
}
