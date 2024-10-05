use actix_web::{HttpResponse, ResponseError};
use anyhow::Error as AnyhowError;

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

impl From<AnyhowError> for AppError {
    fn from(err: AnyhowError) -> Self {
        AppError(err)
    }
}

// If you want to use Result<T, AppError> conveniently
pub type AppResult<T> = std::result::Result<T, AppError>;