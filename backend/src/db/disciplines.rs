// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{Discipline, SyncMode, TimerMode};

#[derive(Serialize, Deserialize)]
struct DisciplineItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    id: Uuid,
    event_id: Uuid,
    name: String,
    mode: TimerMode,
    #[serde(default)]
    sync_mode: SyncMode,
    position: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl DisciplineItem {
    fn from_discipline(d: &Discipline) -> Self {
        Self {
            pk: keys::event_pk(d.event_id),
            sk: keys::event_sk_discipline(d.position, d.id),

            id: d.id,
            event_id: d.event_id,
            name: d.name.clone(),
            mode: d.mode,
            sync_mode: d.sync_mode,
            position: d.position,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }

    fn into_discipline(self) -> Discipline {
        Discipline {
            id: self.id,
            event_id: self.event_id,
            name: self.name,
            mode: self.mode,
            sync_mode: self.sync_mode,
            position: self.position,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, discipline: &Discipline) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(DisciplineItem::from_discipline(discipline))
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

pub async fn delete(client: &DbClient, discipline: &Discipline) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(discipline.event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_discipline(discipline.position, discipline.id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<Discipline>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("DISC#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: DisciplineItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_discipline());
    }
    Ok(out)
}

/// Convenience: scan a discipline by id within an event by listing all
/// disciplines and filtering. The list is bounded (a few entries per
/// event), so this is fine.
pub async fn find(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
) -> AppResult<Option<Discipline>> {
    Ok(list_for_event(client, event_id)
        .await?
        .into_iter()
        .find(|d| d.id == discipline_id))
}

pub async fn set_sync_mode(
    client: &DbClient,
    discipline: &Discipline,
    mode: SyncMode,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let value = serde_dynamo::to_attribute_value(mode)
        .map_err(|e| AppError::Internal(format!("serde_dynamo to_attribute_value: {e}")))?;
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(discipline.event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_discipline(discipline.position, discipline.id)),
        )
        .update_expression("SET sync_mode = :s, updated_at = :u")
        .expression_attribute_values(":s", value)
        .expression_attribute_values(":u", AttributeValue::S(now))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}
