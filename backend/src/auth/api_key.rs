// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Long-lived API keys issued to a User. The desktop standalone app stores
//! one of these and includes it on every export/ingest request.
//!
//! - Plaintext is shown to the operator **once** at creation.
//! - Stored as `argon2id(plaintext)`.
//! - The first 6 chars of the plaintext serve as both a UI label
//!   ("ki1F2g…") and the GSI lookup key, so verification is at most one
//!   `argon2id::verify` per matching prefix (overwhelmingly: exactly one).

use argon2::Argon2;
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::auth::session::random_token;
use crate::db::{api_keys, users};
use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;

const KEY_HEADER: &str = "X-API-Key";

/// 30 bytes of randomness → 40 url-safe-base64 chars without padding.
/// 240 bits of entropy is overkill for the threat model; the cost is
/// negligible.
pub fn generate() -> String {
    random_token(30)
}

pub fn key_prefix(plaintext: &str) -> String {
    plaintext.chars().take(6).collect()
}

pub fn hash(plaintext: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("argon2 hash: {e}")))
}

pub fn verify(plaintext: &str, hashed: &str) -> bool {
    let parsed = match PasswordHash::new(hashed) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(plaintext.as_bytes(), &parsed)
        .is_ok()
}

/// Extractor for routes authenticated by the desktop app's `X-API-Key` header.
pub struct ApiKeyAuth {
    pub user: User,
    pub key_id: String,
}

impl FromRequestParts<AppState> for ApiKeyAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(KEY_HEADER)
            .ok_or(AppError::Unauthorized)?
            .to_str()
            .map_err(|_| AppError::Unauthorized)?;

        let plaintext = header.trim();
        if plaintext.is_empty() {
            return Err(AppError::Unauthorized);
        }
        let prefix = key_prefix(plaintext);

        let candidates = api_keys::list_by_prefix(&state.db, &prefix).await?;
        for cand in candidates {
            if verify(plaintext, &cand.hashed_key) {
                let user = users::get_by_sub(&state.db, &cand.user_sub)
                    .await?
                    .ok_or(AppError::Unauthorized)?;
                return Ok(ApiKeyAuth {
                    user,
                    key_id: cand.key_id,
                });
            }
        }

        Err(AppError::Unauthorized)
    }
}
