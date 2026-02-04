use actix_web::{HttpResponse, ResponseError, error::BlockingError};
use anyhow::anyhow;
use anyhow::Error as AnyhowError;
use base64_url::base64;
use eyre::ErrReport;
use crate::log_error;
use crate::constants::TAG;

pub struct AppError(pub AnyhowError);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Debug for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

// impl<E: Error + Send + Sync + 'static> From<E> for AppError {
//     fn from(error: E) -> Self {
//         AppError(AnyhowError::new(error))
//     }
// }

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        log_error!(TAG, "AppError occurred: {:?}", self);

        // Check if this is a "not ready" error and return 503 instead of 500
        let error_msg = self.0.to_string();
        if error_msg.contains("Backend not ready")
            || error_msg.contains("not initialized")
            || error_msg.contains("Initialization") {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({
                    "error": "Service temporarily unavailable",
                    "message": error_msg,
                    "retry_after": 5
                }));
        }

        HttpResponse::InternalServerError().json(format!("Something went wrong: {error_msg}"))
    }
}

impl From<base64::DecodeError> for AppError {
    fn from(err: base64::DecodeError) -> Self {
        AppError(anyhow::Error::new(err).context("Base64 decoding error"))
    }
}

impl From<std::array::TryFromSliceError> for AppError {
    fn from(err: std::array::TryFromSliceError) -> Self {
        AppError(anyhow::Error::new(err).context("Invalid key length"))
    }
}

impl From<AnyhowError> for AppError {
    fn from(err: AnyhowError) -> Self {
        AppError(err)
    }
}

impl From<Vec<u8>> for AppError {
    fn from(vec: Vec<u8>) -> Self {
        AppError(anyhow!(
            "Invalid key length: expected 32 bytes, got {}",
            vec.len()
        ))
    }
}

impl From<ErrReport> for AppError {
    fn from(err: ErrReport) -> Self {
        AppError(anyhow!(err.to_string()))
    }
}

impl From<BlockingError> for AppError {
    fn from(error: BlockingError) -> Self {
        AppError(AnyhowError::new(error).context("An error occurred during a blocking operation"))
    }
}

// Shortcut for Result<T, AppError>
pub type AppResult<T> = std::result::Result<T, AppError>;
