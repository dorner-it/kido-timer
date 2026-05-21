// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::Runner;

#[derive(Serialize, Deserialize)]
struct RunnerItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    id: Uuid,
    team_id: Uuid,
    event_id: Uuid,
    first_name: String,
    last_name: String,
    birth_date: NaiveDate,
    position: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RunnerItem {
    fn from_runner(r: &Runner) -> Self {
        Self {
            pk: keys::event_pk(r.event_id),
            sk: keys::event_sk_runner(r.team_id, r.position),

            id: r.id,
            team_id: r.team_id,
            event_id: r.event_id,
            first_name: r.first_name.clone(),
            last_name: r.last_name.clone(),
            birth_date: r.birth_date,
            position: r.position,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }

    fn into_runner(self) -> Runner {
        Runner {
            id: self.id,
            team_id: self.team_id,
            event_id: self.event_id,
            first_name: self.first_name,
            last_name: self.last_name,
            birth_date: self.birth_date,
            position: self.position,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, runner: &Runner) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(RunnerItem::from_runner(runner))
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

pub async fn delete(client: &DbClient, runner: &Runner) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(runner.event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_runner(runner.team_id, runner.position)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

pub async fn list_for_team(
    client: &DbClient,
    event_id: Uuid,
    team_id: Uuid,
) -> AppResult<Vec<Runner>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S(format!("RUNNER#{team_id}#")))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<Runner>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("RUNNER#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

fn parse_items(items: Vec<HashMap<String, AttributeValue>>) -> AppResult<Vec<Runner>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: RunnerItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_runner());
    }
    Ok(out)
}
