// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{LineupConfirmedBy, Run, RunStatus, TimeSource};

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
    event_id: Uuid,
    discipline_id: Uuid,
    team_id: Uuid,
    runner_id: Option<Uuid>,
    run_number: u32,
    lane: Option<u8>,
    status: RunStatus,
    original_time_ms: Option<u32>,
    correction_ms: i32,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    // 2027 fields — all defaulted so pre-2027 rows still load.
    #[serde(default)]
    lineup: Vec<Uuid>,
    #[serde(default)]
    lineup_confirmed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    lineup_confirmed_by: Option<LineupConfirmedBy>,
    #[serde(default)]
    time_source: TimeSource,
    #[serde(default)]
    hand_time_1_ms: Option<u32>,
    #[serde(default)]
    hand_time_2_ms: Option<u32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RunItem {
    fn from_run(r: &Run) -> Self {
        Self {
            pk: keys::event_pk(r.event_id),
            sk: keys::event_sk_run(r.discipline_id, r.run_number, r.id),
            gsi1pk: keys::run_status_gsi1pk(r.event_id),
            gsi1sk: keys::run_status_gsi1sk(r.status.as_str(), r.discipline_id, r.run_number),

            id: r.id,
            event_id: r.event_id,
            discipline_id: r.discipline_id,
            team_id: r.team_id,
            runner_id: r.runner_id,
            run_number: r.run_number,
            lane: r.lane,
            status: r.status,
            original_time_ms: r.original_time_ms,
            correction_ms: r.correction_ms,
            started_at: r.started_at,
            ended_at: r.ended_at,
            lineup: r.lineup.clone(),
            lineup_confirmed_at: r.lineup_confirmed_at,
            lineup_confirmed_by: r.lineup_confirmed_by,
            time_source: r.time_source,
            hand_time_1_ms: r.hand_time_1_ms,
            hand_time_2_ms: r.hand_time_2_ms,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }

    fn into_run(self) -> Run {
        Run {
            id: self.id,
            event_id: self.event_id,
            discipline_id: self.discipline_id,
            team_id: self.team_id,
            runner_id: self.runner_id,
            run_number: self.run_number,
            lane: self.lane,
            status: self.status,
            original_time_ms: self.original_time_ms,
            correction_ms: self.correction_ms,
            started_at: self.started_at,
            ended_at: self.ended_at,
            lineup: self.lineup,
            lineup_confirmed_at: self.lineup_confirmed_at,
            lineup_confirmed_by: self.lineup_confirmed_by,
            time_source: self.time_source,
            hand_time_1_ms: self.hand_time_1_ms,
            hand_time_2_ms: self.hand_time_2_ms,
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
    event_id: Uuid,
    discipline_id: Uuid,
    run_number: u32,
    run_id: Uuid,
) -> AppResult<Option<Run>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_run(discipline_id, run_number, run_id)),
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

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<Run>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("RUN#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

pub async fn list_for_discipline(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
) -> AppResult<Vec<Run>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
        .expression_attribute_values(
            ":prefix",
            AttributeValue::S(keys::event_sk_run_prefix_for_discipline(discipline_id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    parse_items(res.items.unwrap_or_default())
}

pub struct TimingUpdate {
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub run_number: u32,
    pub run_id: Uuid,
    pub original_time_ms: Option<u32>,
    pub status: RunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

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
        .key("PK", AttributeValue::S(keys::event_pk(u.event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_run(u.discipline_id, u.run_number, u.run_id)),
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
            AttributeValue::S(keys::run_status_gsi1sk(
                u.status.as_str(),
                u.discipline_id,
                u.run_number,
            )),
        )
        .expression_attribute_values(":u", AttributeValue::S(now.to_rfc3339()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

pub async fn update_correction_ms(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
    run_number: u32,
    run_id: Uuid,
    correction_ms: i32,
) -> AppResult<()> {
    let now = Utc::now();
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(event_id)))
        .key(
            "SK",
            AttributeValue::S(keys::event_sk_run(discipline_id, run_number, run_id)),
        )
        .update_expression("SET correction_ms = :c, updated_at = :u")
        .expression_attribute_values(":c", AttributeValue::N(correction_ms.to_string()))
        .expression_attribute_values(":u", AttributeValue::S(now.to_rfc3339()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

/// Find a run of a discipline by id (the run number, part of the SK, is
/// not known to callers that only have the id).
pub async fn find(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Uuid,
    run_id: Uuid,
) -> AppResult<Option<Run>> {
    Ok(list_for_discipline(client, event_id, discipline_id)
        .await?
        .into_iter()
        .find(|r| r.id == run_id))
}

fn parse_items(items: Vec<HashMap<String, AttributeValue>>) -> AppResult<Vec<Run>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: RunItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_run());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_item() -> HashMap<String, AttributeValue> {
        serde_dynamo::to_item(serde_json::json!({
            "PK": "EVENT#x",
            "SK": "RUN#x",
            "GSI1PK": "EVENT#x#RUN",
            "GSI1SK": "STATUS#completed#x",
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "team_id": Uuid::nil(),
            "runner_id": null,
            "run_number": 1,
            "lane": 1,
            "status": "completed",
            "original_time_ms": 47230,
            "correction_ms": 0,
            "started_at": null,
            "ended_at": null,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        }))
        .unwrap()
    }

    #[test]
    fn legacy_item_defaults_2027_fields() {
        let r = serde_dynamo::from_item::<_, RunItem>(legacy_item())
            .unwrap()
            .into_run();
        assert!(r.lineup.is_empty());
        assert_eq!(r.lineup_confirmed_by, None);
        assert_eq!(r.time_source, TimeSource::Lichtschranke);
        assert_eq!(r.hand_time_1_ms, None);
        assert_eq!(r.original_time_ms, Some(47230));
    }

    #[test]
    fn item_roundtrip_keeps_lineup_and_hand_times() {
        let mut r = serde_dynamo::from_item::<_, RunItem>(legacy_item())
            .unwrap()
            .into_run();
        r.lineup = vec![Uuid::new_v4(), Uuid::new_v4()];
        r.lineup_confirmed_at = Some("2027-06-12T10:00:00Z".parse().unwrap());
        r.lineup_confirmed_by = Some(LineupConfirmedBy::Organizer);
        r.time_source = TimeSource::Handstopp;
        r.hand_time_1_ms = Some(47230);
        r.hand_time_2_ms = Some(47245);
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(RunItem::from_run(&r)).unwrap();
        assert_eq!(
            item.get("time_source"),
            Some(&AttributeValue::S("handstopp".into()))
        );
        let back = serde_dynamo::from_item::<_, RunItem>(item)
            .unwrap()
            .into_run();
        assert_eq!(back, r);
    }
}
