// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! User-scoped settings: API key issuance / list / revoke.
//!
//! Mounted at `/api/users/me/...`. Authenticated via the session cookie.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::auth::api_key;
use crate::db::api_keys;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/me/api-keys", get(list_keys).post(create_key))
        .route("/me/api-keys/{key_id}", delete(revoke_key))
}

/// API representation of an API key. The plaintext is never returned by
/// `list`; it is only included in the `create` response.
#[derive(Serialize)]
pub struct ApiKeySummary {
    pub key_id: String,
    pub name: String,
    pub prefix: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ApiKeyCreatedResponse {
    pub key: ApiKeySummary,
    /// Full plaintext key. Shown to the user **once** at creation; never
    /// retrievable later.
    pub plaintext: String,
}

#[derive(Deserialize)]
pub struct CreateApiKeyInput {
    pub name: Option<String>,
}

pub async fn list_keys(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> AppResult<Json<Vec<ApiKeySummary>>> {
    let records = api_keys::list_for_user(&state.db, &auth.user.sub).await?;
    let mut out: Vec<ApiKeySummary> = records
        .into_iter()
        .map(|r| ApiKeySummary {
            key_id: r.key_id,
            name: r.name,
            prefix: r.prefix,
            created_at: r.created_at,
        })
        .collect();
    out.sort_by_key(|k| std::cmp::Reverse(k.created_at));
    Ok(Json(out))
}

pub async fn create_key(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(input): Json<CreateApiKeyInput>,
) -> AppResult<(StatusCode, Json<ApiKeyCreatedResponse>)> {
    let name = input
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Desktop".to_string());

    let plaintext = api_key::generate();
    let prefix = api_key::key_prefix(&plaintext);
    let hashed_key = api_key::hash(&plaintext)?;
    let key_id = Uuid::new_v4().to_string();
    let created_at = Utc::now();

    let rec = api_keys::ApiKeyRecord {
        key_id: key_id.clone(),
        user_sub: auth.user.sub,
        name: name.clone(),
        prefix: prefix.clone(),
        hashed_key,
        created_at,
    };
    api_keys::put(&state.db, &rec).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyCreatedResponse {
            key: ApiKeySummary {
                key_id,
                name,
                prefix,
                created_at,
            },
            plaintext,
        }),
    ))
}

pub async fn revoke_key(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(key_id): Path<String>,
) -> AppResult<StatusCode> {
    // Make sure the key belongs to this user before deleting.
    let owns = api_keys::list_for_user(&state.db, &auth.user.sub)
        .await?
        .into_iter()
        .any(|k| k.key_id == key_id);
    if !owns {
        return Err(AppError::NotFound(format!("api key {key_id}")));
    }
    api_keys::delete(&state.db, &auth.user.sub, &key_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
