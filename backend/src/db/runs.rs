// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{Run, RunStatus};

#[derive(Serialize, Deserialize)]
struct RunItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,
    #[serde(rename = "GSI1PK")]
    gsi1pk: String,
    #[serde(rename = "GSI1SK")]
    gsi1sk: String,

    id: Uuid,
    competition_id: Uuid,
    team_id: Uuid,
    runner_id: Option<Uuid>,
    run_number: u32,
    lane: Option<u8>,
    status: RunStatus,
    original_time_ms: Option<u32>,
    correction_ms: i32,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RunItem {
    fn from_run(r: &Run) -> Self {
        Self {
            pk: keys::competition_pk(r.competition_id),
            sk: keys::competition_sk_run(r.run_number, r.id),
            gsi1pk: keys::run_status_gsi1pk(r.competition_id),
            gsi1sk: keys::run_status_gsi1sk(r.status.as_str(), r.run_number),

            id: r.id,
            competition_id: r.competition_id,
            team_id: r.team_id,
            runner_id: r.runner_id,
            run_number: r.run_number,
            lane: r.lane,
            status: r.status,
            original_time_ms: r.original_time_ms,
            correction_ms: r.correction_ms,
            started_at: r.started_at,
            ended_at: r.ended_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }

    fn into_run(self) -> Run {
        Run {
            id: self.id,
            competition_id: self.competition_id,
            team_id: self.team_id,
            runner_id: self.runner_id,
            run_number: self.run_number,
            lane: self.lane,
            status: self.status,
            original_time_ms: self.original_time_ms,
            correction_ms: self.correction_ms,
            started_at: self.started_at,
            ended_at: self.ended_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, run: &Run) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(RunItem::from_run(run))
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
    competition_id: Uuid,
    run_number: u32,
    run_id: Uuid,
) -> AppResult<Option<Run>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(competition_id)))
        .key(
            "SK",
            AttributeValue::S(keys::competition_sk_run(run_number, run_id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };
    let parsed: RunItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
    Ok(Some(parsed.into_run()))
}

pub async fn list_for_competition(
    client: &DbClient,
    competition_id: Uuid,
) -> AppResult<Vec<Run>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::competition_pk(competition_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("RUN#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut runs = Vec::with_capacity(items.len());
    for item in items {
        let parsed: RunItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        runs.push(parsed.into_run());
    }
    Ok(runs)
}

pub struct TimingUpdate {
    pub competition_id: Uuid,
    pub run_number: u32,
    pub run_id: Uuid,
    pub original_time_ms: Option<u32>,
    pub status: RunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// Apply a timing update from the desktop ingest endpoint. Touches only
/// `original_time_ms`, `status`, `started_at`, `ended_at`, plus the GSI1SK
/// (which is derived from `status` and `run_number`) and `updated_at` —
/// `correction_ms`, `team_id`, `runner_id`, `lane` are preserved.
pub async fn update_timing(client: &DbClient, u: TimingUpdate) -> AppResult<()> {
    let now = Utc::now();
    let started = match u.started_at {
        Some(t) => AttributeValue::S(t.to_rfc3339()),
        None => AttributeValue::Null(true),
    };
    let ended = match u.ended_at {
        Some(t) => AttributeValue::S(t.to_rfc3339()),
        None => AttributeValue::Null(true),
    };
    let original = match u.original_time_ms {
        Some(ms) => AttributeValue::N(ms.to_string()),
        None => AttributeValue::Null(true),
    };

    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(u.competition_id)))
        .key(
            "SK",
            AttributeValue::S(keys::competition_sk_run(u.run_number, u.run_id)),
        )
        .update_expression(
            "SET original_time_ms = :o, #s = :st, started_at = :sa, ended_at = :ea, GSI1SK = :gsk, updated_at = :u",
        )
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":o", original)
        .expression_attribute_values(":st", AttributeValue::S(u.status.as_str().to_string()))
        .expression_attribute_values(":sa", started)
        .expression_attribute_values(":ea", ended)
        .expression_attribute_values(
            ":gsk",
            AttributeValue::S(keys::run_status_gsi1sk(u.status.as_str(), u.run_number)),
        )
        .expression_attribute_values(":u", AttributeValue::S(now.to_rfc3339()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

/// Update only the `correction_ms` and `updated_at` fields. Used after a
/// penalty is approved/rejected so we don't have to re-write the whole row.
pub async fn update_correction_ms(
    client: &DbClient,
    competition_id: Uuid,
    run_number: u32,
    run_id: Uuid,
    correction_ms: i32,
) -> AppResult<()> {
    let now = Utc::now();
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::competition_pk(competition_id)))
        .key(
            "SK",
            AttributeValue::S(keys::competition_sk_run(run_number, run_id)),
        )
        .update_expression("SET correction_ms = :c, updated_at = :u")
        .expression_attribute_values(":c", AttributeValue::N(correction_ms.to_string()))
        .expression_attribute_values(":u", AttributeValue::S(now.to_rfc3339()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}
