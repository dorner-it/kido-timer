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
