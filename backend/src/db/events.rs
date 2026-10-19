// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::Event;
use crate::models::event::normalize_current_runs;

#[derive(Serialize, Deserialize)]
struct EventItem {
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
    is_active: bool,
    current_discipline_id: Option<Uuid>,
    #[serde(default)]
    current_run_id: Option<Uuid>,
    /// Missing on pre-2027 rows → `[current_run_id]` (or `[]`).
    #[serde(default)]
    current_run_ids: Option<Vec<Uuid>>,
    decider_tokens: [String; 4],
    #[serde(default)]
    station_token: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl EventItem {
    fn from_event(e: &Event) -> Self {
        Self {
            pk: keys::event_pk(e.id),
            sk: keys::SK_META.to_string(),
            gsi1pk: keys::user_owned_events_gsi1pk(&e.owner_sub),
            gsi1sk: keys::user_events_gsi1sk(e.date, e.id),

            id: e.id,
            owner_sub: e.owner_sub.clone(),
            name: e.name.clone(),
            date: e.date,
            location: e.location.clone(),
            is_active: e.is_active,
            current_discipline_id: e.current_discipline_id,
            current_run_id: e.current_run_ids.first().copied(),
            current_run_ids: Some(e.current_run_ids.clone()),
            decider_tokens: e.decider_tokens.clone(),
            station_token: e.station_token.clone(),
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }

    fn into_event(self) -> Event {
        let (current_run_id, current_run_ids) =
            normalize_current_runs(self.current_run_id, self.current_run_ids);
        Event {
            id: self.id,
            owner_sub: self.owner_sub,
            name: self.name,
            date: self.date,
            location: self.location,
            is_active: self.is_active,
            current_discipline_id: self.current_discipline_id,
            current_run_id,
            current_run_ids,
            decider_tokens: self.decider_tokens,
            station_token: self.station_token,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub async fn put(client: &DbClient, event: &Event) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(EventItem::from_event(event))
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

pub async fn get(client: &DbClient, id: Uuid) -> AppResult<Option<Event>> {
    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };
    let parsed: EventItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
    Ok(Some(parsed.into_event()))
}

pub async fn delete(client: &DbClient, id: Uuid) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

/// Delete the event META row plus every child row (Discipline, Team,
/// TeamEntry, Runner, Run, Penalty) underneath it. Two phases: query
/// everything under `PK = EVENT#<id>`, then chunked BatchWriteItem
/// deletes (DDB caps each batch at 25).
pub async fn cascade_delete(client: &DbClient, id: Uuid) -> AppResult<()> {
    use aws_sdk_dynamodb::types::{DeleteRequest, WriteRequest};

    let pk = keys::event_pk(id);
    let mut last_evaluated: Option<HashMap<String, AttributeValue>> = None;
    let mut all_keys: Vec<(String, String)> = Vec::new();

    loop {
        let res = client
            .inner
            .query()
            .table_name(&client.table_name)
            .key_condition_expression("PK = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()))
            .projection_expression("PK, SK")
            .set_exclusive_start_key(last_evaluated.clone())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

        for item in res.items.unwrap_or_default() {
            let pk_val = item
                .get("PK")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            let sk_val = item
                .get("SK")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            if !pk_val.is_empty() && !sk_val.is_empty() {
                all_keys.push((pk_val, sk_val));
            }
        }

        last_evaluated = res.last_evaluated_key;
        if last_evaluated.is_none() {
            break;
        }
    }

    for chunk in all_keys.chunks(25) {
        let mut requests: Vec<WriteRequest> = Vec::with_capacity(chunk.len());
        for (pk_val, sk_val) in chunk {
            let delete_req = DeleteRequest::builder()
                .key("PK", AttributeValue::S(pk_val.clone()))
                .key("SK", AttributeValue::S(sk_val.clone()))
                .build()
                .map_err(|e| AppError::Internal(format!("delete request: {e}")))?;
            requests.push(WriteRequest::builder().delete_request(delete_req).build());
        }

        client
            .inner
            .batch_write_item()
            .request_items(client.table_name.clone(), requests)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb batch_write: {e}")))?;
    }

    Ok(())
}

/// Sets `is_active = true` on the target event and `false` on every other
/// event the same owner has. Not transactional but acceptable for v1 with
/// at most a few dozen events per user.
pub async fn activate_exclusive(
    client: &DbClient,
    owner_sub: &str,
    target_id: Uuid,
) -> AppResult<()> {
    let events = list_for_owner(client, owner_sub).await?;
    for e in events {
        let should_be_active = e.id == target_id;
        if e.is_active == should_be_active {
            continue;
        }
        client
            .inner
            .update_item()
            .table_name(&client.table_name)
            .key("PK", AttributeValue::S(keys::event_pk(e.id)))
            .key("SK", AttributeValue::S(keys::SK_META.to_string()))
            .update_expression("SET is_active = :a, updated_at = :u")
            .expression_attribute_values(":a", AttributeValue::Bool(should_be_active))
            .expression_attribute_values(":u", AttributeValue::S(Utc::now().to_rfc3339()))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    }
    Ok(())
}

/// Set the discipline on the lane and its current runs. Writes both the
/// 2027 `current_run_ids` list and the legacy `current_run_id`
/// (= first element) so old readers keep working.
pub async fn set_current(
    client: &DbClient,
    event_id: Uuid,
    discipline_id: Option<Uuid>,
    run_ids: &[Uuid],
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let disc_value = match discipline_id {
        Some(id) => AttributeValue::S(id.to_string()),
        None => AttributeValue::Null(true),
    };
    let run_value = match run_ids.first() {
        Some(id) => AttributeValue::S(id.to_string()),
        None => AttributeValue::Null(true),
    };
    let run_ids_value = AttributeValue::L(
        run_ids
            .iter()
            .map(|id| AttributeValue::S(id.to_string()))
            .collect(),
    );
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(event_id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .update_expression(
            "SET current_discipline_id = :d, current_run_id = :r, current_run_ids = :rs, updated_at = :u",
        )
        .expression_attribute_values(":d", disc_value)
        .expression_attribute_values(":r", run_value)
        .expression_attribute_values(":rs", run_ids_value)
        .expression_attribute_values(":u", AttributeValue::S(now))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

pub async fn rotate_decider_tokens(
    client: &DbClient,
    event_id: Uuid,
    tokens: [String; 4],
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    let av = AttributeValue::L(tokens.into_iter().map(AttributeValue::S).collect());
    client
        .inner
        .update_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(keys::event_pk(event_id)))
        .key("SK", AttributeValue::S(keys::SK_META.to_string()))
        .update_expression("SET decider_tokens = :t, updated_at = :u")
        .expression_attribute_values(":t", av)
        .expression_attribute_values(":u", AttributeValue::S(now))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb update_item: {e}")))?;
    Ok(())
}

pub async fn list_for_owner(client: &DbClient, owner_sub: &str) -> AppResult<Vec<Event>> {
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
            AttributeValue::S(keys::user_owned_events_gsi1pk(owner_sub)),
        )
        .expression_attribute_values(":prefix", AttributeValue::S("EVENT#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut events = Vec::with_capacity(items.len());
    for item in items {
        let parsed: EventItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        events.push(parsed.into_event());
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_item(current_run_id: Option<Uuid>) -> HashMap<String, AttributeValue> {
        serde_dynamo::to_item(serde_json::json!({
            "PK": "EVENT#x",
            "SK": "META",
            "GSI1PK": "USER#sub",
            "GSI1SK": "EVENT#2027-06-12#x",
            "id": Uuid::nil(),
            "owner_sub": "sub",
            "name": "Kreisausscheid",
            "date": "2027-06-12",
            "location": null,
            "is_active": true,
            "current_discipline_id": null,
            "current_run_id": current_run_id,
            "decider_tokens": ["a", "b", "c", "d"],
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        }))
        .unwrap()
    }

    #[test]
    fn legacy_item_reads_current_run_as_list() {
        let run = Uuid::new_v4();
        let e = serde_dynamo::from_item::<_, EventItem>(legacy_item(Some(run)))
            .unwrap()
            .into_event();
        assert_eq!(e.current_run_ids, vec![run]);
        assert_eq!(e.current_run_id, Some(run));
        assert_eq!(e.station_token, "");
    }

    #[test]
    fn legacy_item_without_current_run_reads_empty() {
        let e = serde_dynamo::from_item::<_, EventItem>(legacy_item(None))
            .unwrap()
            .into_event();
        assert!(e.current_run_ids.is_empty());
        assert_eq!(e.current_run_id, None);
    }

    #[test]
    fn item_writes_both_current_run_fields() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let mut e = serde_dynamo::from_item::<_, EventItem>(legacy_item(None))
            .unwrap()
            .into_event();
        e.set_current_runs(vec![a, b]);
        let item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(EventItem::from_event(&e)).unwrap();
        assert_eq!(
            item.get("current_run_id"),
            Some(&AttributeValue::S(a.to_string()))
        );
        assert_eq!(
            item.get("current_run_ids"),
            Some(&AttributeValue::L(vec![
                AttributeValue::S(a.to_string()),
                AttributeValue::S(b.to_string()),
            ]))
        );
        let back = serde_dynamo::from_item::<_, EventItem>(item)
            .unwrap()
            .into_event();
        assert_eq!(back, e);
    }
}
