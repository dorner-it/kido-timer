// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation: {0}")]
    Validation(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("rate limited")]
    RateLimited,

    /// Wertungsgruppe could not be derived (mixed gender outside WG1,
    /// roster too old, missing gender, …). Surfaced as 400 with code
    /// `wg_inconsistent` so the frontend can flag the offending entry
    /// rather than swallow it under a generic `validation` error.
    #[error("wg_inconsistent: {0}")]
    WgInconsistent(String),

    #[error("internal: {0}")]
    Internal(String),
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            AppError::WgInconsistent(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "not_found",
            AppError::Validation(_) => "validation",
            AppError::Unauthorized => "unauthorized",
            AppError::Forbidden => "forbidden",
            AppError::Conflict(_) => "conflict",
            AppError::RateLimited => "rate_limited",
            AppError::WgInconsistent(_) => "wg_inconsistent",
            AppError::Internal(_) => "internal",
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if matches!(self, AppError::Internal(_)) {
            tracing::error!(error = %self, "internal error");
        }

        let status = self.status();
        let public_message = match &self {
            AppError::Internal(_) => "internal server error".to_string(),
            other => other.to_string(),
        };
        let code = self.code();

        (
            status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code,
                    message: public_message,
                },
            }),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
