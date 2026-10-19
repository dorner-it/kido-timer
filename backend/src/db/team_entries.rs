// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{TeamEntry, Wertungsgruppe, WgHistoryEntry};

#[derive(Serialize, Deserialize)]
struct TeamEntryItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    team_id: Uuid,
    discipline_id: Uuid,
    event_id: Uuid,
    start_number: u32,
    lane: Option<u8>,
    /// Tolerates rows written before the `wg` migration.
    #[serde(default)]
    wg: Option<Wertungsgruppe>,
    /// Append-only "Umsetzen" log; missing on pre-2027 rows.
    #[serde(default)]
    wg_history: Vec<WgHistoryEntry>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TeamEntryItem {
    fn from_entry(e: &TeamEntry) -> Self {
        Self {
            pk: keys::event_pk(e.event_id),
            sk: keys::event_sk_team_entry(e.discipline_id, e.start_number, e.team_id),

            team_id: e.team_id,
            discipline_id: e.discipline_id,
            event_id: e.event_id,
            start_number: e.start_number,
            lane: e.lane,
            wg: e.wg,
            wg_history: e.wg_history.clone(),
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }

    fn into_entry(self) -> TeamEntry {
        TeamEntry {
            team_id: self.team_id,
            discipline_id: self.discipline_id,
            event_id: self.event_id,
            start_number: self.start_number,
            lane: self.lane,
            wg: self.wg,
            wg_history: self.wg_history,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, entry: &TeamEntry) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(TeamEntryItem::from_entry(entry))
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

pub async fn delete(client: &DbClient, entry: &TeamEntry) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(entry.event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_team_entry(
                entry.discipline_id,
                entry.start_number,
                entry.team_id,
            )),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<TeamEntry>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("ENTRY#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

pub async fn list_for_discipline(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
) -> AppResult<Vec<TeamEntry>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(
            ":prefix",
            AttributeValue::S(format!("ENTRY#{discipline_id}#")),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

fn parse_items(items: Vec<HashMap<String, AttributeValue>>) -> AppResult<Vec<TeamEntry>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: TeamEntryItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_entry());
    }
    Ok(out)
}

/// Find the entry of one team in one discipline.
pub async fn find(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
    team_id: Uuid,
) -> AppResult<Option<TeamEntry>> {
    Ok(list_for_discipline(client, event_id, discipline_id)
        .await?
        .into_iter()
        .find(|e| e.team_id == team_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_item(wg: Option<&str>) -> HashMap<String, AttributeValue> {
        let mut j = serde_json::json!({
            "PK": "EVENT#x",
            "SK": "ENTRY#x",
            "team_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "start_number": 4,
            "lane": null,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        });
        if let Some(wg) = wg {
            j["wg"] = serde_json::json!(wg);
        }
        serde_dynamo::to_item(j).unwrap()
    }

    #[test]
    fn pre_wg_item_reads_null_wg_and_empty_history() {
        let e = serde_dynamo::from_item::<_, TeamEntryItem>(legacy_item(None))
            .unwrap()
            .into_entry();
        assert_eq!(e.wg, None);
        assert!(e.wg_history.is_empty());
    }

    #[test]
    fn derived_wg_item_reads_wg() {
        let e = serde_dynamo::from_item::<_, TeamEntryItem>(legacy_item(Some("wg4")))
            .unwrap()
            .into_entry();
        assert_eq!(e.wg, Some(Wertungsgruppe::Wg4));
    }

    #[test]
    fn item_roundtrip_keeps_history() {
        let mut e = serde_dynamo::from_item::<_, TeamEntryItem>(legacy_item(Some("wg2")))
            .unwrap()
            .into_entry();
        e.wg = Some(Wertungsgruppe::Wg1);
        e.wg_history.push(WgHistoryEntry {
            from: Some(Wertungsgruppe::Wg2),
            to: Wertungsgruppe::Wg1,
            by: "fbl@example.org".into(),
            at: "2027-06-12T09:00:00Z".parse().unwrap(),
            note: Some("2 Mädchen".into()),
        });
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(TeamEntryItem::from_entry(&e)).unwrap();
        let back = serde_dynamo::from_item::<_, TeamEntryItem>(item)
            .unwrap()
            .into_entry();
        assert_eq!(back, e);
    }
}
