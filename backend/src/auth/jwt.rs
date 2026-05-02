// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use serde::Deserialize;

use crate::auth::jwks::JwksCache;
use crate::error::{AppError, AppResult};

/// Claims from a Cognito-issued ID token. Cognito's actual token has more
/// fields; we deserialize only what we use.
#[derive(Debug, Deserialize)]
pub struct CognitoIdClaims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "cognito:username", default)]
    pub cognito_username: Option<String>,
    pub exp: i64,
}

pub async fn validate_id_token(
    token: &str,
    jwks: &JwksCache,
    issuer: &str,
    audience: &str,
) -> AppResult<CognitoIdClaims> {
    let header = decode_header(token).map_err(|_| AppError::Unauthorized)?;
    let kid = header.kid.ok_or(AppError::Unauthorized)?;
    let key = jwks.key_for(&kid).await?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);

    let data = decode::<CognitoIdClaims>(token, &key, &validation)
        .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}
