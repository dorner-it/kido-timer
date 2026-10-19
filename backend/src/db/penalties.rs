// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::penalty::migrate_kind;
use crate::models::{Penalty, PenaltyKind, PenaltyStatus, StationKind};

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
    #[serde(default)]
    decider_slot: u8,
    #[serde(default)]
    station: Option<StationKind>,
    /// Missing on pre-2027 rows → `is_disqualification ? fv : sz`.
    #[serde(default)]
    kind: Option<PenaltyKind>,
    /// Catalogue code; missing on pre-2027 rows.
    #[serde(default)]
    code: Option<String>,
    /// Still written (= `kind == fv`) so pre-2027 readers keep working.
    #[serde(default)]
    is_disqualification: bool,
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
            station: p.station,
            kind: Some(p.kind),
            code: p.code.clone(),
            is_disqualification: p.kind == PenaltyKind::Fv,
            delta_ms: p.delta_ms,
            reason_code: p.reason_code.clone(),
            status: p.status,
            created_at: p.created_at,
            reviewed_at: p.reviewed_at,
            reviewed_by: p.reviewed_by.clone(),
        }
    }

    fn into_penalty(self) -> Penalty {
        let kind = migrate_kind(self.kind, self.is_disqualification);
        Penalty {
            id: self.id,
            event_id: self.event_id,
            discipline_id: self.discipline_id,
            run_id: self.run_id,
            team_id: self.team_id,
            runner_id: self.runner_id,
            kind,
            code: self.code,
            station: self.station,
            is_disqualification: kind == PenaltyKind::Fv,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_item(extra: serde_json::Value) -> HashMap<String, AttributeValue> {
        let mut j = serde_json::json!({
            "PK": "EVENT#x",
            "SK": "PENALTY#x",
            "GSI1PK": "EVENT#x#PENALTY",
            "GSI1SK": "STATUS#approved#2026-05-29T12:00:00Z",
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "run_id": Uuid::nil(),
            "team_id": Uuid::nil(),
            "runner_id": null,
            "decider_slot": 1,
            "delta_ms": 5000,
            "reason_code": null,
            "status": "approved",
            "created_at": "2026-05-29T12:00:00Z",
            "reviewed_at": null,
            "reviewed_by": null
        });
        if let (Some(base), Some(add)) = (j.as_object_mut(), extra.as_object()) {
            for (k, v) in add {
                base.insert(k.clone(), v.clone());
            }
        }
        serde_dynamo::to_item(j).unwrap()
    }

    #[test]
    fn pre_stations_item_reads_as_sz() {
        let p = serde_dynamo::from_item::<_, PenaltyItem>(legacy_item(serde_json::json!({})))
            .unwrap()
            .into_penalty();
        assert_eq!(p.kind, PenaltyKind::Sz);
        assert_eq!(p.code, None);
        assert_eq!(p.station, None);
        assert!(!p.is_disqualification);
        assert_eq!(p.delta_ms, 5000);
        assert_eq!(p.status, PenaltyStatus::Approved);
    }

    #[test]
    fn legacy_dq_item_reads_as_fv_with_canonical_station() {
        let p = serde_dynamo::from_item::<_, PenaltyItem>(legacy_item(serde_json::json!({
            "station": "strahlrohr",
            "is_disqualification": true,
            "delta_ms": 999000
        })))
        .unwrap()
        .into_penalty();
        assert_eq!(p.kind, PenaltyKind::Fv);
        assert!(p.is_disqualification);
        assert_eq!(p.station, Some(StationKind::Strahlrohrlinie));
        assert_eq!(p.delta_ms, 999_000);
    }

    #[test]
    fn item_without_decider_slot_loads() {
        let mut item = legacy_item(serde_json::json!({}));
        item.remove("decider_slot");
        let p = serde_dynamo::from_item::<_, PenaltyItem>(item)
            .unwrap()
            .into_penalty();
        assert_eq!(p.decider_slot, 0);
    }

    #[test]
    fn new_item_roundtrips_kind_code_and_canonical_station() {
        let p = serde_dynamo::from_item::<_, PenaltyItem>(legacy_item(serde_json::json!({
            "station": "knotengestell",
            "kind": "sz",
            "code": "knotengestell.ein_knoten_falsch",
            "delta_ms": 15000,
            "decider_slot": 0
        })))
        .unwrap()
        .into_penalty();
        assert_eq!(p.code.as_deref(), Some("knotengestell.ein_knoten_falsch"));
        // Writing back needs PenaltyStatus::as_str (GSI key) from kido-core.
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(PenaltyItem::from_penalty(&p)).unwrap();
        assert_eq!(item.get("kind"), Some(&AttributeValue::S("sz".into())));
        assert_eq!(
            item.get("station"),
            Some(&AttributeValue::S("knotengestell".into()))
        );
        assert_eq!(
            item.get("is_disqualification"),
            Some(&AttributeValue::Bool(false))
        );
        let back = serde_dynamo::from_item::<_, PenaltyItem>(item)
            .unwrap()
            .into_penalty();
        assert_eq!(back, p);
    }
}
