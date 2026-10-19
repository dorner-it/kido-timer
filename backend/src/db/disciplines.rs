// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{Discipline, JudgeMode, SyncMode, TimerMode};

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
    /// Missing on pre-2027 rows → `mode.default_lane_count()`.
    #[serde(default)]
    lane_count: Option<u8>,
    /// Missing on pre-2027 rows → `per_station`.
    #[serde(default)]
    judge_mode: Option<JudgeMode>,
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
            lane_count: Some(d.lane_count),
            judge_mode: Some(d.judge_mode),
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
            lane_count: self
                .lane_count
                .unwrap_or_else(|| self.mode.default_lane_count()),
            judge_mode: self.judge_mode.unwrap_or_default(),
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
            AttributeValue::S(keys::event_sk_discipline(
                discipline.position,
                discipline.id,
            )),
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
            AttributeValue::S(keys::event_sk_discipline(
                discipline.position,
                discipline.id,
            )),
        )
        .update_expression("SET sync_mode = :s, updated_at = :u")
        .expression_attribute_values(":s", value)
        .expression_attribute_values(":u", AttributeValue::S(now))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_item() -> HashMap<String, AttributeValue> {
        serde_dynamo::to_item(serde_json::json!({
            "PK": "EVENT#x",
            "SK": "DISC#0001#x",
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "name": "Gruppenstafette",
            "mode": "gruppenstaffette",
            "position": 1,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        }))
        .unwrap()
    }

    #[test]
    fn legacy_item_loads_with_defaults() {
        let mut item = legacy_item();
        // Pin lane_count so the test does not depend on kido-core.
        item.insert("lane_count".into(), AttributeValue::N("2".into()));
        let parsed: DisciplineItem = serde_dynamo::from_item(item).unwrap();
        let d = parsed.into_discipline();
        assert_eq!(d.lane_count, 2);
        assert_eq!(d.judge_mode, JudgeMode::PerStation);
        assert_eq!(d.sync_mode, SyncMode::Live);
    }

    #[test]
    fn legacy_item_without_lane_count_uses_mode_default() {
        let parsed: DisciplineItem = serde_dynamo::from_item(legacy_item()).unwrap();
        assert_eq!(parsed.into_discipline().lane_count, 2);
    }

    #[test]
    fn item_roundtrip_keeps_new_fields() {
        let mut item = legacy_item();
        item.insert("lane_count".into(), AttributeValue::N("1".into()));
        item.insert(
            "judge_mode".into(),
            AttributeValue::S("per_station_and_lane".into()),
        );
        let d = serde_dynamo::from_item::<_, DisciplineItem>(item)
            .unwrap()
            .into_discipline();
        let again: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(DisciplineItem::from_discipline(&d)).unwrap();
        assert_eq!(
            again.get("lane_count"),
            Some(&AttributeValue::N("1".into()))
        );
        assert_eq!(
            again.get("judge_mode"),
            Some(&AttributeValue::S("per_station_and_lane".into()))
        );
        let back = serde_dynamo::from_item::<_, DisciplineItem>(again)
            .unwrap()
            .into_discipline();
        assert_eq!(back, d);
    }
}
