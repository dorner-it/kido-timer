// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;

use crate::auth::session::COOKIE_NAME;
use crate::db::{sessions, users};
use crate::error::AppError;
use crate::models::User;
use crate::state::AppState;

/// Authenticated organizer extractor for axum handlers.
///
/// Resolves the session cookie → DDB session row → DDB user row.
/// Returns 401 if any step fails. Tokens are not exposed; only the
/// hydrated `User` is.
pub struct AuthenticatedUser {
    pub user: User,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let cookie = jar.get(COOKIE_NAME).ok_or(AppError::Unauthorized)?;

        let session = sessions::get(&state.db, cookie.value())
            .await?
            .ok_or(AppError::Unauthorized)?;

        let user = users::get_by_sub(&state.db, &session.sub)
            .await?
            .ok_or(AppError::Unauthorized)?;

        Ok(AuthenticatedUser { user })
    }
}
