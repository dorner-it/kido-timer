// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{Competition, TimerMode};

#[derive(Serialize, Deserialize)]
struct CompetitionItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,
    #[serde(rename = "GSI1PK")]
    gsi1pk: String,
    #[serde(rename = "GSI1SK")]
    gsi1sk: String,

    id: Uuid,
    owner_sub: String,
    name: String,
    date: NaiveDate,
    location: Option<String>,
    mode: TimerMode,
    is_active: bool,
    current_run_id: Option<Uuid>,
    decider_tokens: [String; 4],
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CompetitionItem {
    fn from_competition(c: &Competition) -> Self {
        Self {
            pk: keys::competition_pk(c.id),
            sk: keys::SK_META.to_string(),
            gsi1pk: keys::user_owned_competitions_gsi1pk(&c.owner_sub),
            gsi1sk: keys::user_competitions_gsi1sk(c.date, c.id),

            id: c.id,
            owner_sub: c.owner_sub.clone(),
            name: c.name.clone(),
            date: c.date,
            location: c.location.clone(),
            mode: c.mode,
            is_active: c.is_active,
            current_run_id: c.current_run_id,
            decider_tokens: c.decider_tokens.clone(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }

    fn into_competition(self) -> Competition {
        Competition {
            id: self.id,
            owner_sub: self.owner_sub,
            name: self.name,
            date: self.date,
            location: self.location,
            mode: self.mode,
            is_active: self.is_active,
            current_run_id: self.current_run_id,
            decider_tokens: self.decider_tokens,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, competition: &Competition) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(CompetitionItem::from_competition(competition))
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

pub async fn get(client: &DbClient, id: Uuid) -> AppResult<Option<Competition>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };

    let parsed: CompetitionItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
    Ok(Some(parsed.into_competition()))
}

pub async fn delete(client: &DbClient, id: Uuid) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

/// Delete the competition META row plus every Team / Runner / Run / Penalty
/// row underneath it. Two phases: query everything under PK = COMP#<id>,
/// then chunked BatchWriteItem deletes (DDB caps each batch at 25).
pub async fn cascade_delete(client: &DbClient, id: Uuid) -> AppResult<()> {
    use aws_sdk_dynamodb::types::{DeleteRequest, WriteRequest};

    let pk = keys::competition_pk(id);
    let mut last_evaluated: Option<HashMap<String, AttributeValue>> = None;
    let mut all_keys: Vec<(String, String)> = Vec::new();

    loop {
        let res = client
            .inner
            .query()
            .table_name(&client.table_name)
            .key_condition_expression("PK = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()))
            .projection_expression("PK, SK")
            .set_exclusive_start_key(last_evaluated.clone())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

        for item in res.items.unwrap_or_default() {
            let pk_val = item
                .get("PK")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            let sk_val = item
                .get("SK")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            if !pk_val.is_empty() && !sk_val.is_empty() {
                all_keys.push((pk_val, sk_val));
            }
        }

        last_evaluated = res.last_evaluated_key;
        if last_evaluated.is_none() {
            break;
        }
    }

    for chunk in all_keys.chunks(25) {
        let mut requests: Vec<WriteRequest> = Vec::with_capacity(chunk.len());
        for (pk_val, sk_val) in chunk {
            let delete_req = DeleteRequest::builder()
                .key("PK", AttributeValue::S(pk_val.clone()))
                .key("SK", AttributeValue::S(sk_val.clone()))
                .build()
                .map_err(|e| AppError::Internal(format!("delete request: {e}")))?;
            requests.push(WriteRequest::builder().delete_request(delete_req).build());
        }

        client
            .inner
            .batch_write_item()
            .request_items(client.table_name.clone(), requests)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb batch_write: {e}")))?;
    }

    Ok(())
}

/// Sets is_active=true on the target competition and is_active=false on every
/// other competition the same owner has. Two-phase update; not transactional,
/// but acceptable for v1 with at most a few dozen competitions per user.
pub async fn activate_exclusive(
    client: &DbClient,
    owner_sub: &str,
    target_id: Uuid,
) -> AppResult<()> {
    let competitions = list_for_owner(client, owner_sub).await?;
    for c in competitions {
        let should_be_active = c.id == target_id;
        if c.is_active == should_be_active {
            continue;
        }
        client
            .inner
            .update_item()
            .table_name(&client.table_name)
            .key("PK", AttributeValue::S(keys::competition_pk(c.id)))
            .key("SK", AttributeValue::S(keys::SK_META.to_string()))
            .update_expression("SET is_active = :a, updated_at = :u")
            .expression_attribute_values(":a", AttributeValue::Bool(should_be_active))
            .expression_attribute_values(":u", AttributeValue::S(Utc::now().to_rfc3339()))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    }
    Ok(())
}

pub async fn set_current_run(
    client: &DbClient,
    competition_id: Uuid,
    run_id: Option<Uuid>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let value = match run_id {
        Some(id) => AttributeValue::S(id.to_string()),
        None => AttributeValue::Null(true),
    };
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(competition_id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .update_expression("SET current_run_id = :r, updated_at = :u")
        .expression_attribute_values(":r", value)
        .expression_attribute_values(":u", AttributeValue::S(now))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

pub async fn rotate_decider_tokens(
    client: &DbClient,
    competition_id: Uuid,
    tokens: [String; 4],
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let av = AttributeValue::L(tokens.into_iter().map(AttributeValue::S).collect());
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(competition_id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .update_expression("SET decider_tokens = :t, updated_at = :u")
        .expression_attribute_values(":t", av)
        .expression_attribute_values(":u", AttributeValue::S(now))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

pub async fn list_for_owner(client: &DbClient, owner_sub: &str) -> AppResult<Vec<Competition>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .index_name("GSI1")
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", "GSI1PK")
        .expression_attribute_names("#sk", "GSI1SK")
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(keys::user_owned_competitions_gsi1pk(owner_sub)),
        )
        .expression_attribute_values(":prefix", AttributeValue::S("COMP#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut competitions = Vec::with_capacity(items.len());
    for item in items {
        let parsed: CompetitionItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        competitions.push(parsed.into_competition());
    }
    Ok(competitions)
}
