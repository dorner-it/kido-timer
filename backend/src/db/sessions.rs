// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::client::DbClient;
use crate::error::{AppError, AppResult};

/// Server-side session row keyed by an opaque session id stored in an httpOnly
/// cookie. Refresh tokens never reach the browser.
///
/// Auto-purged by DynamoDB TTL — `ttl` is a unix-epoch-seconds attribute.
#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub sub: String,
    pub refresh_token: String,
    pub created_at: DateTime<Utc>,
    pub ttl: i64,
}

#[derive(Serialize, Deserialize)]
struct SessionItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    session_id: String,
    sub: String,
    refresh_token: String,
    created_at: DateTime<Utc>,
    ttl: i64,
}

fn pk(session_id: &str) -> String {
    format!("SESSION#{session_id}")
}

const SK: &str = "META";

pub async fn put(client: &DbClient, session: &Session) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(SessionItem {
        pk: pk(&session.session_id),
        sk: SK.to_string(),
        session_id: session.session_id.clone(),
        sub: session.sub.clone(),
        refresh_token: session.refresh_token.clone(),
        created_at: session.created_at,
        ttl: session.ttl,
    })
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

pub async fn get(client: &DbClient, session_id: &str) -> AppResult<Option<Session>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(pk(session_id)))
        .key("SK", AttributeValue::S(SK.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };

    let parsed: SessionItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;

    if parsed.ttl < Utc::now().timestamp() {
        return Ok(None);
    }

    Ok(Some(Session {
        session_id: parsed.session_id,
        sub: parsed.sub,
        refresh_token: parsed.refresh_token,
        created_at: parsed.created_at,
        ttl: parsed.ttl,
    }))
}

pub async fn delete(client: &DbClient, session_id: &str) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(pk(session_id)))
        .key("SK", AttributeValue::S(SK.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}
