// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{Penalty, PenaltyStatus};

#[derive(Serialize, Deserialize)]
struct PenaltyItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,
    #[serde(rename = "GSI1PK")]
    gsi1pk: String,
    #[serde(rename = "GSI1SK")]
    gsi1sk: String,

    id: Uuid,
    event_id: Uuid,
    discipline_id: Uuid,
    run_id: Uuid,
    team_id: Uuid,
    runner_id: Option<Uuid>,
    decider_slot: u8,
    delta_ms: i32,
    reason_code: Option<String>,
    status: PenaltyStatus,
    created_at: DateTime<Utc>,
    reviewed_at: Option<DateTime<Utc>>,
    reviewed_by: Option<String>,
}

impl PenaltyItem {
    fn from_penalty(p: &Penalty) -> Self {
        Self {
            pk: keys::event_pk(p.event_id),
            sk: keys::event_sk_penalty(p.discipline_id, p.id),
            gsi1pk: keys::penalty_status_gsi1pk(p.event_id),
            gsi1sk: keys::penalty_status_gsi1sk(p.status.as_str(), p.created_at),

            id: p.id,
            event_id: p.event_id,
            discipline_id: p.discipline_id,
            run_id: p.run_id,
            team_id: p.team_id,
            runner_id: p.runner_id,
            decider_slot: p.decider_slot,
            delta_ms: p.delta_ms,
            reason_code: p.reason_code.clone(),
            status: p.status,
            created_at: p.created_at,
            reviewed_at: p.reviewed_at,
            reviewed_by: p.reviewed_by.clone(),
        }
    }

    fn into_penalty(self) -> Penalty {
        Penalty {
            id: self.id,
            event_id: self.event_id,
            discipline_id: self.discipline_id,
            run_id: self.run_id,
            team_id: self.team_id,
            runner_id: self.runner_id,
            decider_slot: self.decider_slot,
            delta_ms: self.delta_ms,
            reason_code: self.reason_code,
            status: self.status,
            created_at: self.created_at,
            reviewed_at: self.reviewed_at,
            reviewed_by: self.reviewed_by,
        }
    }
}

pub async fn put(client: &DbClient, penalty: &Penalty) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(PenaltyItem::from_penalty(penalty))
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

pub async fn get(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
    penalty_id: Uuid,
) -> AppResult<Option<Penalty>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_penalty(discipline_id, penalty_id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };
    let parsed: PenaltyItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
    Ok(Some(parsed.into_penalty()))
}

pub async fn delete(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
    penalty_id: Uuid,
) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_penalty(discipline_id, penalty_id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<Penalty>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("PENALTY#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

pub async fn list_for_discipline(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
) -> AppResult<Vec<Penalty>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(
            ":prefix",
            AttributeValue::S(keys::event_sk_penalty_prefix_for_discipline(discipline_id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

pub async fn list_by_status(
    client: &DbClient,
    event_id: Uuid,
    status: PenaltyStatus,
) -> AppResult<Vec<Penalty>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .index_name("GSI1")
        .key_condition_expression("GSI1PK = :pk AND begins_with(GSI1SK, :prefix)")
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(keys::penalty_status_gsi1pk(event_id)),
        )
        .expression_attribute_values(
            ":prefix",
            AttributeValue::S(format!("STATUS#{}#", status.as_str())),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

fn parse_items(items: Vec<HashMap<String, AttributeValue>>) -> AppResult<Vec<Penalty>> {
    let mut penalties = Vec::with_capacity(items.len());
    for item in items {
        let parsed: PenaltyItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        penalties.push(parsed.into_penalty());
    }
    Ok(penalties)
}
