// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::User;

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
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl UserItem {
    fn from_user(u: &User) -> Self {
        Self {
            pk: keys::user_pk(&u.sub),
            sk: keys::SK_PROFILE.to_string(),
            gsi1pk: keys::user_email_gsi1pk(&u.email),
            gsi1sk: keys::user_pk(&u.sub),

            sub: u.sub.clone(),
            email: u.email.to_lowercase(),
            display_name: u.display_name.clone(),
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }

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

pub async fn put(client: &DbClient, user: &User) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(UserItem::from_user(user))
        .map_err(|e| AppError::Internal(format!("serde_dynamo to_item: {e}")))?;

    client
        .inner
        .put_item()
        .table_name(&client.table_name)
        .set_item(Some(item))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb put_item: {e}")))?;
    Ok(())
}

pub async fn get_by_sub(client: &DbClient, sub: &str) -> AppResult<Option<User>> {
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
    Ok(Some(parsed.into_user()))
}

/// Upsert a user record. Used on Cognito callback after token validation —
/// idempotent: re-running for an existing user just refreshes the email and
/// display_name from the latest claims.
pub async fn upsert_from_claims(
    client: &DbClient,
    sub: &str,
    email: &str,
    display_name: &str,
) -> AppResult<User> {
    let now = Utc::now();
    let existing = get_by_sub(client, sub).await?;

    let user = User {
        sub: sub.to_string(),
        email: email.to_lowercase(),
        display_name: display_name.to_string(),
        created_at: existing.as_ref().map(|u| u.created_at).unwrap_or(now),
        updated_at: now,
    };

    put(client, &user).await?;
    Ok(user)
}
