// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::session::random_token;
use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::User;

/// 32 bytes of randomness, base64url-encoded → 43 chars. Used as the secret
/// key for HMAC-SHA256 signatures on offline export/import envelopes.
fn random_hmac_key() -> String {
    random_token(32)
}

#[derive(Serialize, Deserialize)]
struct UserItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,
    #[serde(rename = "GSI1PK")]
    gsi1pk: String,
    #[serde(rename = "GSI1SK")]
    gsi1sk: String,

    sub: String,
    email: String,
    display_name: String,
    /// Per-user HMAC-SHA256 key. Server-only; never serialised back to API
    /// clients. `#[serde(default)]` so existing items written before this
    /// field deserialise into an empty key, and we backfill on next write.
    #[serde(default)]
    hmac_key: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl UserItem {
    fn into_user(self) -> User {
        User {
            sub: self.sub,
            email: self.email,
            display_name: self.display_name,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

async fn get_item_by_sub(client: &DbClient, sub: &str) -> AppResult<Option<UserItem>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::user_pk(sub)))
        .key("SK", AttributeValue::S(keys::SK_PROFILE.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };
    let parsed: UserItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
    Ok(Some(parsed))
}

async fn put_item(client: &DbClient, item: &UserItem) -> AppResult<()> {
    let av: HashMap<String, AttributeValue> = serde_dynamo::to_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo to_item: {e}")))?;
    client
        .inner
        .put_item()
        .table_name(&client.table_name)
        .set_item(Some(av))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb put_item: {e}")))?;
    Ok(())
}

pub async fn get_by_sub(client: &DbClient, sub: &str) -> AppResult<Option<User>> {
    Ok(get_item_by_sub(client, sub).await?.map(UserItem::into_user))
}

/// Returns the user's per-user HMAC key. Errors if the user does not exist
/// or if the key has not yet been generated (it is generated lazily during
/// `upsert_from_claims`, i.e. on every sign-in, so this should be present
/// for any signed-in user).
pub async fn get_hmac_key(client: &DbClient, sub: &str) -> AppResult<String> {
    let item = get_item_by_sub(client, sub)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("user {sub}")))?;
    if item.hmac_key.is_empty() {
        return Err(AppError::Internal(
            "user has no HMAC key; sign in once to backfill".into(),
        ));
    }
    Ok(item.hmac_key)
}

/// Upsert a user record. Used on Cognito callback after token validation —
/// idempotent: re-running for an existing user just refreshes the email and
/// display_name from the latest claims, preserving created_at and the
/// per-user HMAC key.
pub async fn upsert_from_claims(
    client: &DbClient,
    sub: &str,
    email: &str,
    display_name: &str,
) -> AppResult<User> {
    let now = Utc::now();
    let existing = get_item_by_sub(client, sub).await?;

    let (created_at, hmac_key) = match existing {
        Some(prev) => {
            let key = if prev.hmac_key.is_empty() {
                random_hmac_key()
            } else {
                prev.hmac_key
            };
            (prev.created_at, key)
        }
        None => (now, random_hmac_key()),
    };

    let item = UserItem {
        pk: keys::user_pk(sub),
        sk: keys::SK_PROFILE.to_string(),
        gsi1pk: keys::user_email_gsi1pk(email),
        gsi1sk: keys::user_pk(sub),
        sub: sub.to_string(),
        email: email.to_lowercase(),
        display_name: display_name.to_string(),
        hmac_key,
        created_at,
        updated_at: now,
    };

    put_item(client, &item).await?;
    Ok(item.into_user())
}
