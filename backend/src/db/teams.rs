// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::Team;

#[derive(Serialize, Deserialize)]
struct TeamItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    id: Uuid,
    event_id: Uuid,
    name: String,
    position: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TeamItem {
    fn from_team(t: &Team) -> Self {
        Self {
            pk: keys::event_pk(t.event_id),
            sk: keys::event_sk_team(t.position, t.id),

            id: t.id,
            event_id: t.event_id,
            name: t.name.clone(),
            position: t.position,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }

    fn into_team(self) -> Team {
        Team {
            id: self.id,
            event_id: self.event_id,
            name: self.name,
            position: self.position,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, team: &Team) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(TeamItem::from_team(team))
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

pub async fn delete(client: &DbClient, team: &Team) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(team.event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_team(team.position, team.id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<Team>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("TEAM#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut teams = Vec::with_capacity(items.len());
    for item in items {
        let parsed: TeamItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        teams.push(parsed.into_team());
    }
    Ok(teams)
}
