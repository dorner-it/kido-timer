// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::{AttributeValue, Delete, TransactWriteItem};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::db::runners::StoredRunner;
use crate::error::{AppError, AppResult};
use crate::models::{Team, TeamEntry};

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

// ===== Team delete cascade =====

/// DynamoDB caps a transaction at 100 items.
const MAX_TRANSACT_ITEMS: usize = 100;

/// One conditional delete of the team cascade: remove `sk` only if the
/// item's attribute `attr` still equals `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeDelete {
    pub sk: String,
    pub attr: &'static str,
    pub value: String,
}

/// Plan the deletes for removing a team: its runners (every stored row,
/// new **and** legacy key, as read — conditional on the runner `id`), its
/// Meldungen in every discipline (conditional on `team_id`), and the team
/// item itself (conditional on `id`) **last**.
///
/// Runs and penalties are *not* touched: they are the competition record
/// (times, approved penalties) and already render with a fallback name
/// for an unknown team.
///
/// `runners` / `entries` may contain rows of other teams; they are
/// filtered out here.
pub fn plan_delete_cascade(
    team: &Team,
    runners: &[StoredRunner],
    entries: &[TeamEntry],
) -> Vec<CascadeDelete> {
    let mut ops: Vec<CascadeDelete> = runners
        .iter()
        .filter(|r| r.runner.team_id == team.id)
        .map(|r| CascadeDelete {
            sk: r.sk.clone(),
            attr: "id",
            value: r.runner.id.to_string(),
        })
        .collect();
    ops.extend(
        entries
            .iter()
            .filter(|e| e.team_id == team.id)
            .map(|e| CascadeDelete {
                sk: keys::event_sk_team_entry(e.discipline_id, e.start_number, e.team_id),
                attr: "team_id",
                value: team.id.to_string(),
            }),
    );
    // Dedupe (a row can't be deleted twice in one transaction).
    let mut seen = std::collections::HashSet::new();
    ops.retain(|op| seen.insert(op.sk.clone()));
    ops.push(CascadeDelete {
        sk: keys::event_sk_team(team.position, team.id),
        attr: "id",
        value: team.id.to_string(),
    });
    ops
}

/// Split the plan into transactions of at most 100 deletes. The team item
/// is the last op, so it lands in the last chunk: if an earlier chunk
/// fails, the team still exists and a retry (which re-reads the remaining
/// rows) finishes the job.
pub fn chunk_cascade(ops: &[CascadeDelete]) -> Vec<&[CascadeDelete]> {
    ops.chunks(MAX_TRANSACT_ITEMS).collect()
}

/// Execute [`plan_delete_cascade`]: one `TransactWriteItems` per chunk of
/// 100. Chunks are atomic each, the whole cascade only for teams with
/// fewer than 100 runners + Meldungen (the normal case). A failed
/// condition (row changed or vanished since it was read) cancels the
/// chunk → `409 Conflict`; nothing of that chunk is written.
pub async fn delete_cascade(
    client: &DbClient,
    event_id: Uuid,
    ops: &[CascadeDelete],
) -> AppResult<()> {
    let pk = keys::event_pk(event_id);
    for chunk in chunk_cascade(ops) {
        let mut items = Vec::with_capacity(chunk.len());
        for op in chunk {
            let delete = Delete::builder()
                .table_name(&client.table_name)
                .key("PK", AttributeValue::S(pk.clone()))
                .key("SK", AttributeValue::S(op.sk.clone()))
                .condition_expression("#a = :v")
                .expression_attribute_names("#a", op.attr)
                .expression_attribute_values(":v", AttributeValue::S(op.value.clone()))
                .build()
                .map_err(|e| AppError::Internal(format!("transact delete: {e}")))?;
            items.push(TransactWriteItem::builder().delete(delete).build());
        }
        client
            .inner
            .transact_write_items()
            .set_transact_items(Some(items))
            .send()
            .await
            .map_err(|e| {
                if e.as_service_error()
                    .is_some_and(|se| se.is_transaction_canceled_exception())
                {
                    AppError::Conflict(
                        "Mannschaft wurde zwischenzeitlich geändert – bitte neu laden".into(),
                    )
                } else {
                    AppError::Internal(format!("dynamodb transact_write_items: {e}"))
                }
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! In-memory model of one event partition: SK → attributes. Deletes
    //! check their condition; a transaction is all-or-nothing.

    use std::collections::BTreeMap;

    use chrono::{NaiveDate, TimeZone};

    use super::*;
    use crate::models::Runner;
    use crate::models::wertungsgruppe::Wertungsgruppe;

    const EVENT: Uuid = Uuid::from_u128(0xe);
    const TEAM: Uuid = Uuid::from_u128(0x7);
    const OTHER: Uuid = Uuid::from_u128(0x8);
    const DISC1: Uuid = Uuid::from_u128(0xd1);
    const DISC2: Uuid = Uuid::from_u128(0xd2);

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 19, 8, 0, 0).unwrap()
    }

    fn team(id: Uuid, position: u32) -> Team {
        Team {
            id,
            event_id: EVENT,
            name: "FF".into(),
            position,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn runner(id: u128, team_id: Uuid, position: u32) -> Runner {
        Runner {
            id: Uuid::from_u128(id),
            team_id,
            event_id: EVENT,
            first_name: "X".into(),
            last_name: "Y".into(),
            birth_date: NaiveDate::from_ymd_opt(2014, 1, 1).unwrap(),
            position,
            gender: None,
            created_at: ts(),
            updated_at: ts(),
        }
    }

    fn entry(team_id: Uuid, discipline_id: Uuid, start: u32) -> TeamEntry {
        TeamEntry {
            team_id,
            discipline_id,
            event_id: EVENT,
            start_number: start,
            lane: None,
            wg: Some(Wertungsgruppe::Wg1),
            wg_history: vec![],
            created_at: ts(),
            updated_at: ts(),
        }
    }

    #[derive(Default)]
    struct Table {
        items: BTreeMap<String, HashMap<&'static str, String>>,
        runners: Vec<StoredRunner>,
        entries: Vec<TeamEntry>,
    }

    impl Table {
        fn add_team(&mut self, t: &Team) {
            self.items.insert(
                keys::event_sk_team(t.position, t.id),
                HashMap::from([("id", t.id.to_string())]),
            );
        }
        fn add_runner(&mut self, r: Runner, legacy: bool) {
            let sk = if legacy {
                keys::event_sk_runner_legacy(r.team_id, r.position)
            } else {
                keys::event_sk_runner(r.team_id, r.position, r.id)
            };
            self.items.insert(
                sk.clone(),
                HashMap::from([("id", r.id.to_string()), ("team_id", r.team_id.to_string())]),
            );
            self.runners.push(StoredRunner { sk, runner: r });
        }
        fn add_entry(&mut self, e: TeamEntry) {
            self.items.insert(
                keys::event_sk_team_entry(e.discipline_id, e.start_number, e.team_id),
                HashMap::from([("team_id", e.team_id.to_string())]),
            );
            self.entries.push(e);
        }
        fn transact(&mut self, ops: &[CascadeDelete]) -> Result<(), &'static str> {
            assert!(ops.len() <= MAX_TRANSACT_ITEMS);
            let ok = ops.iter().all(|op| {
                self.items
                    .get(&op.sk)
                    .and_then(|attrs| attrs.get(op.attr))
                    .is_some_and(|v| *v == op.value)
            });
            if !ok {
                return Err("TransactionCanceled");
            }
            for op in ops {
                self.items.remove(&op.sk);
            }
            Ok(())
        }
        fn delete_team(&mut self, t: &Team) -> Result<(), &'static str> {
            let ops = plan_delete_cascade(t, &self.runners, &self.entries);
            for chunk in chunk_cascade(&ops) {
                self.transact(chunk)?;
            }
            Ok(())
        }
        fn keys_with(&self, needle: Uuid) -> usize {
            let n = needle.to_string();
            self.items.keys().filter(|k| k.contains(&n)).count()
        }
    }

    fn fixture() -> (Table, Team, Team) {
        let mut tbl = Table::default();
        let t = team(TEAM, 1);
        let o = team(OTHER, 2);
        tbl.add_team(&t);
        tbl.add_team(&o);
        tbl.add_runner(runner(1, TEAM, 1), false);
        tbl.add_runner(runner(2, TEAM, 2), true); // legacy key
        tbl.add_runner(runner(3, OTHER, 1), false);
        tbl.add_runner(runner(4, OTHER, 2), true); // legacy key, same position as runner 2
        tbl.add_entry(entry(TEAM, DISC1, 1));
        tbl.add_entry(entry(TEAM, DISC2, 4));
        tbl.add_entry(entry(OTHER, DISC1, 2));
        (tbl, t, o)
    }

    #[test]
    fn deletes_team_runners_of_both_key_formats_and_entries() {
        let (mut tbl, t, _) = fixture();
        tbl.delete_team(&t).unwrap();
        assert_eq!(tbl.keys_with(TEAM), 0, "no row of the team left");
    }

    #[test]
    fn other_team_is_untouched() {
        let (mut tbl, t, o) = fixture();
        tbl.delete_team(&t).unwrap();
        // team item + 2 runners + 1 entry
        assert_eq!(tbl.keys_with(OTHER), 4);
        assert!(
            tbl.items
                .contains_key(&keys::event_sk_team(o.position, o.id))
        );
        assert_eq!(tbl.items.len(), 4);
    }

    #[test]
    fn plan_conditions_and_team_last() {
        let (tbl, t, _) = fixture();
        let ops = plan_delete_cascade(&t, &tbl.runners, &tbl.entries);
        assert_eq!(ops.len(), 5); // 2 runners + 2 entries + team
        let last = ops.last().unwrap();
        assert_eq!(last.sk, keys::event_sk_team(1, TEAM));
        assert_eq!((last.attr, last.value.clone()), ("id", TEAM.to_string()));
        for op in &ops[..2] {
            assert_eq!(op.attr, "id");
            assert!(
                op.sk
                    .starts_with(&keys::event_sk_runner_prefix_for_team(TEAM))
            );
        }
        for op in &ops[2..4] {
            assert_eq!((op.attr, op.value.clone()), ("team_id", TEAM.to_string()));
            assert!(op.sk.starts_with("ENTRY#"));
        }
    }

    #[test]
    fn team_without_runners_or_entries_deletes_only_the_team() {
        let mut tbl = Table::default();
        let t = team(TEAM, 3);
        tbl.add_team(&t);
        let ops = plan_delete_cascade(&t, &tbl.runners, &tbl.entries);
        assert_eq!(ops.len(), 1);
        tbl.delete_team(&t).unwrap();
        assert!(tbl.items.is_empty());
    }

    #[test]
    fn legacy_runner_key_held_by_another_runner_cancels_instead_of_deleting_it() {
        // Stale read: the plan thinks runner 2 is at the legacy key, but
        // that key now holds a different runner → the condition fails and
        // nothing is deleted.
        let (mut tbl, t, _) = fixture();
        let legacy = keys::event_sk_runner_legacy(TEAM, 2);
        tbl.items.insert(
            legacy.clone(),
            HashMap::from([("id", Uuid::from_u128(99).to_string())]),
        );
        let before = tbl.items.len();
        assert!(tbl.delete_team(&t).is_err());
        assert_eq!(tbl.items.len(), before);
        assert!(tbl.items.contains_key(&legacy));
    }

    #[test]
    fn large_team_is_chunked_with_team_in_last_chunk() {
        let mut tbl = Table::default();
        let t = team(TEAM, 1);
        tbl.add_team(&t);
        for i in 0..150u32 {
            tbl.add_runner(runner(1000 + u128::from(i), TEAM, i + 1), false);
        }
        tbl.add_entry(entry(TEAM, DISC1, 1));
        let ops = plan_delete_cascade(&t, &tbl.runners, &tbl.entries);
        let chunks = chunk_cascade(&ops);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| c.len() <= MAX_TRANSACT_ITEMS));
        assert_eq!(chunks[1].last().unwrap().sk, keys::event_sk_team(1, TEAM));
        tbl.delete_team(&t).unwrap();
        assert!(tbl.items.is_empty());
    }

    #[test]
    fn failed_first_chunk_keeps_the_team() {
        let mut tbl = Table::default();
        let t = team(TEAM, 1);
        tbl.add_team(&t);
        for i in 0..120u32 {
            tbl.add_runner(runner(1000 + u128::from(i), TEAM, i + 1), false);
        }
        // One runner vanished since the read → first chunk cancels.
        let gone = tbl.runners[0].sk.clone();
        tbl.items.remove(&gone);
        assert!(tbl.delete_team(&t).is_err());
        assert!(tbl.items.contains_key(&keys::event_sk_team(1, TEAM)));
    }

    #[test]
    fn duplicate_rows_in_input_are_deleted_once() {
        let (tbl, t, _) = fixture();
        let mut runners = tbl.runners.clone();
        runners.extend(tbl.runners.clone());
        let ops = plan_delete_cascade(&t, &runners, &tbl.entries);
        assert_eq!(ops.len(), 5);
    }
}
