// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};

#[derive(Serialize, Deserialize, Clone)]
pub struct ApiKeyRecord {
    pub key_id: String,
    pub user_sub: String,
    pub name: String,
    pub prefix: String,
    pub hashed_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct ApiKeyItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,
    #[serde(rename = "GSI1PK")]
    gsi1pk: String,
    #[serde(rename = "GSI1SK")]
    gsi1sk: String,

    key_id: String,
    user_sub: String,
    name: String,
    prefix: String,
    hashed_key: String,
    created_at: DateTime<Utc>,
}

impl ApiKeyItem {
    fn from_record(rec: &ApiKeyRecord) -> Self {
        Self {
            pk: keys::user_pk(&rec.user_sub),
            sk: keys::user_sk_apikey(&rec.key_id),
            gsi1pk: format!("APIKEY#{}", rec.prefix),
            gsi1sk: keys::user_pk(&rec.user_sub),

            key_id: rec.key_id.clone(),
            user_sub: rec.user_sub.clone(),
            name: rec.name.clone(),
            prefix: rec.prefix.clone(),
            hashed_key: rec.hashed_key.clone(),
            created_at: rec.created_at,
        }
    }

    fn into_record(self) -> ApiKeyRecord {
        ApiKeyRecord {
            key_id: self.key_id,
            user_sub: self.user_sub,
            name: self.name,
            prefix: self.prefix,
            hashed_key: self.hashed_key,
            created_at: self.created_at,
        }
    }
}

pub async fn put(client: &DbClient, rec: &ApiKeyRecord) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(ApiKeyItem::from_record(rec))
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

pub async fn list_for_user(client: &DbClient, user_sub: &str) -> AppResult<Vec<ApiKeyRecord>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", "PK")
        .expression_attribute_names("#sk", "SK")
        .expression_attribute_values(":pk", AttributeValue::S(keys::user_pk(user_sub)))
        .expression_attribute_values(":prefix", AttributeValue::S("APIKEY#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: ApiKeyItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_record());
    }
    Ok(out)
}

/// Fetch every API-key row whose plaintext shares the given 6-char prefix.
/// Used by the X-API-Key extractor to find candidates before the
/// argon2 verification.
pub async fn list_by_prefix(client: &DbClient, prefix: &str) -> AppResult<Vec<ApiKeyRecord>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .index_name("GSI1")
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", "GSI1PK")
        .expression_attribute_values(":pk", AttributeValue::S(format!("APIKEY#{prefix}")))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: ApiKeyItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_record());
    }
    Ok(out)
}

pub async fn delete(client: &DbClient, user_sub: &str, key_id: &str) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::user_pk(user_sub)))
        .key("SK", AttributeValue::S(keys::user_sk_apikey(key_id)))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}
