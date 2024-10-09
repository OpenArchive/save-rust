use actix_web::{HttpResponse, ResponseError};
use anyhow::anyhow;
use anyhow::Error as AnyhowError;
use base64_url::base64;
use eyre::ErrReport;

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

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError().json(format!("Something went wrong: {}", self.0))
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
        AppError(anyhow!("Invalid key length: expected 32 bytes, got {}", vec.len()))
    }
}

impl From<ErrReport> for AppError {
    fn from(err: ErrReport) -> Self {
        AppError(anyhow!(err.to_string()))
    }
}

// Shortcut for Result<T, AppError>
pub type AppResult<T> = std::result::Result<T, AppError>;