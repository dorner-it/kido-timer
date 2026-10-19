// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Runner rows.
//!
//! Key: `PK = EVENT#<event_id>`, `SK = RUNNER#<team_id>#<position:04>#<id>`
//! (see [`keys::event_sk_runner`]). Rows written before the id became part
//! of the key (`RUNNER#<team_id>#<position:04>`, "legacy") stay readable —
//! both shapes match the `RUNNER#<team_id>#` query prefix — and are moved to
//! the new key on their next write. Nothing in the request path scans or
//! batch-migrates; `db::migrations::migrate_runner_keys` is the optional
//! operator tool for that.
//!
//! Writes are planned as pure [`RunnerWrite`] lists (unit-testable without
//! DynamoDB) and then executed: a position change is one
//! `TransactWriteItems`, so a swap never leaves a runner missing or doubled.

use std::collections::{HashMap, HashSet};

use aws_sdk_dynamodb::types::{AttributeValue, Delete, Put, TransactWriteItem};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::{Gender, Runner};

/// DynamoDB caps a transaction at 100 items.
const MAX_TRANSACT_ITEMS: usize = 100;

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
    /// Tolerates rows written before the `gender` migration.
    #[serde(default)]
    gender: Option<Gender>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RunnerItem {
    fn from_runner(r: &Runner) -> Self {
        Self {
            pk: keys::event_pk(r.event_id),
            sk: keys::event_sk_runner(r.team_id, r.position, r.id),

            id: r.id,
            team_id: r.team_id,
            event_id: r.event_id,
            first_name: r.first_name.clone(),
            last_name: r.last_name.clone(),
            birth_date: r.birth_date,
            position: r.position,
            gender: r.gender,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }

    fn into_stored(self) -> StoredRunner {
        StoredRunner {
            sk: self.sk,
            runner: Runner {
                id: self.id,
                team_id: self.team_id,
                event_id: self.event_id,
                first_name: self.first_name,
                last_name: self.last_name,
                birth_date: self.birth_date,
                position: self.position,
                gender: self.gender,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
        }
    }
}

/// A runner row together with the sort key it is actually stored under
/// (new or legacy shape).
#[derive(Debug, Clone)]
pub struct StoredRunner {
    pub sk: String,
    pub runner: Runner,
}

impl StoredRunner {
    /// `true` if the row already sits under the id-bearing key.
    pub fn has_current_key(&self) -> bool {
        self.sk == current_sk(&self.runner)
    }
}

/// One planned write against the runner rows of an event.
#[derive(Debug, Clone)]
pub enum RunnerWrite {
    /// Unconditional put under [`keys::event_sk_runner`] — the key contains
    /// the runner id, so it can only ever replace this runner's own row.
    Put(Runner),
    /// Delete `sk`, but only if the row there belongs to `expect_id`
    /// (`ConditionExpression id = :id`). This is what makes deleting a
    /// legacy (position-only) key safe: it can never remove another runner.
    Delete { sk: String, expect_id: Uuid },
}

fn current_sk(r: &Runner) -> String {
    keys::event_sk_runner(r.team_id, r.position, r.id)
}

// ===== Pure planning =====

/// Plan the writes for `PUT …/runners/{id}`.
///
/// `stored` is the team's roster as read (all rows, any key shape),
/// `updated` the runner with its new field values.
///
/// - Every stored row of the runner that is not its new key is deleted
///   (old position and/or legacy key), then the runner is put.
/// - **Swap:** if the position changes and other runners of the team
///   already hold the target position, each of them takes the runner's
///   old position (rows moved the same way, `updated_at = now`).
///
/// All writes are meant to be executed atomically ([`apply_atomic`]).
pub fn plan_update(
    stored: &[StoredRunner],
    updated: &Runner,
    now: DateTime<Utc>,
) -> AppResult<Vec<RunnerWrite>> {
    let own: Vec<&StoredRunner> = stored
        .iter()
        .filter(|s| s.runner.id == updated.id)
        .collect();
    // Same row `normalize` would report (current key wins over legacy).
    let Some(current) = own
        .iter()
        .find(|s| s.has_current_key())
        .or_else(|| own.first())
    else {
        return Err(AppError::NotFound(format!("runner {}", updated.id)));
    };
    let old_position = current.runner.position;

    let mut writes = Vec::new();
    push_move(&mut writes, &own, updated.clone());

    if updated.position != old_position {
        let mut seen = HashSet::new();
        for d in stored.iter().filter(|s| {
            s.runner.id != updated.id
                && s.runner.team_id == updated.team_id
                && s.runner.position == updated.position
        }) {
            if !seen.insert(d.runner.id) {
                continue;
            }
            let rows: Vec<&StoredRunner> = stored
                .iter()
                .filter(|s| s.runner.id == d.runner.id)
                .collect();
            let mut moved = d.runner.clone();
            moved.position = old_position;
            moved.updated_at = now;
            push_move(&mut writes, &rows, moved);
        }
    }

    if writes.len() > MAX_TRANSACT_ITEMS {
        return Err(AppError::Validation(format!(
            "too many runners at position {}",
            updated.position
        )));
    }
    Ok(writes)
}

/// Delete every stored row of a runner that is not `target`'s key, then
/// put `target`.
fn push_move(writes: &mut Vec<RunnerWrite>, rows: &[&StoredRunner], target: Runner) {
    let target_sk = current_sk(&target);
    let mut deleted = HashSet::new();
    for row in rows {
        if row.sk != target_sk && deleted.insert(row.sk.clone()) {
            writes.push(RunnerWrite::Delete {
                sk: row.sk.clone(),
                expect_id: row.runner.id,
            });
        }
    }
    writes.push(RunnerWrite::Put(target));
}

/// Deletes for removing a runner without knowing its stored key shape:
/// the new key and the legacy key, both conditional on the runner id.
/// Executed by [`delete`], which treats a failed condition as "not there".
pub fn delete_ops(runner: &Runner) -> Vec<RunnerWrite> {
    vec![
        RunnerWrite::Delete {
            sk: current_sk(runner),
            expect_id: runner.id,
        },
        RunnerWrite::Delete {
            sk: keys::event_sk_runner_legacy(runner.team_id, runner.position),
            expect_id: runner.id,
        },
    ]
}

/// Turn stored rows into the API list: one entry per runner id (a row
/// under the current key wins over a leftover legacy row), ordered by
/// team, position, then last/first name — positions are not unique.
pub fn normalize(rows: Vec<StoredRunner>) -> Vec<Runner> {
    let mut by_id: HashMap<Uuid, StoredRunner> = HashMap::with_capacity(rows.len());
    for row in rows {
        let keep_existing = by_id
            .get(&row.runner.id)
            .is_some_and(|existing| existing.has_current_key() || !row.has_current_key());
        if !keep_existing {
            by_id.insert(row.runner.id, row);
        }
    }
    let mut out: Vec<Runner> = by_id.into_values().map(|s| s.runner).collect();
    out.sort_by(|a, b| {
        a.team_id
            .cmp(&b.team_id)
            .then(a.position.cmp(&b.position))
            .then_with(|| a.last_name.cmp(&b.last_name))
            .then_with(|| a.first_name.cmp(&b.first_name))
            .then(a.id.cmp(&b.id))
    });
    out
}

// ===== DynamoDB execution =====

fn to_item(runner: &Runner) -> AppResult<HashMap<String, AttributeValue>> {
    serde_dynamo::to_item(RunnerItem::from_runner(runner))
        .map_err(|e| AppError::Internal(format!("serde_dynamo to_item: {e}")))
}

/// Write a runner under its (unique) key. Used by `create`: a new runner
/// at an occupied position simply lands next to the existing one.
pub async fn put(client: &DbClient, runner: &Runner) -> AppResult<()> {
    client
        .inner
        .put_item()
        .table_name(&client.table_name)
        .set_item(Some(to_item(runner)?))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb put_item: {e}")))?;
    Ok(())
}

/// Execute planned writes for one event in a single `TransactWriteItems`
/// (all-or-nothing). A failed `id = :id` condition means the roster
/// changed since it was read → `409 Conflict`, nothing written.
pub async fn apply_atomic(
    client: &DbClient,
    event_id: Uuid,
    writes: &[RunnerWrite],
) -> AppResult<()> {
    match writes {
        [] => return Ok(()),
        [RunnerWrite::Put(r)] => return put(client, r).await,
        _ => {}
    }
    let pk = keys::event_pk(event_id);
    let mut items = Vec::with_capacity(writes.len());
    for w in writes {
        let item = match w {
            RunnerWrite::Put(r) => TransactWriteItem::builder()
                .put(
                    Put::builder()
                        .table_name(&client.table_name)
                        .set_item(Some(to_item(r)?))
                        .build()
                        .map_err(|e| AppError::Internal(format!("transact put: {e}")))?,
                )
                .build(),
            RunnerWrite::Delete { sk, expect_id } => TransactWriteItem::builder()
                .delete(
                    Delete::builder()
                        .table_name(&client.table_name)
                        .key("PK", AttributeValue::S(pk.clone()))
                        .key("SK", AttributeValue::S(sk.clone()))
                        .condition_expression("id = :id")
                        .expression_attribute_values(
                            ":id",
                            AttributeValue::S(expect_id.to_string()),
                        )
                        .build()
                        .map_err(|e| AppError::Internal(format!("transact delete: {e}")))?,
                )
                .build(),
        };
        items.push(item);
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
    Ok(())
}

/// Remove a runner whether it is stored under the new or the legacy key.
/// Both deletes are conditional on the runner id; a failed condition
/// (row absent or owned by another runner) is "not there" and ignored.
pub async fn delete(client: &DbClient, runner: &Runner) -> AppResult<()> {
    let pk = keys::event_pk(runner.event_id);
    for op in delete_ops(runner) {
        let RunnerWrite::Delete { sk, expect_id } = op else {
            continue;
        };
        let res = client
            .inner
            .delete_item()
            .table_name(&client.table_name)
            .key("PK", AttributeValue::S(pk.clone()))
            .key("SK", AttributeValue::S(sk))
            .condition_expression("id = :id")
            .expression_attribute_values(":id", AttributeValue::S(expect_id.to_string()))
            .send()
            .await;
        match res {
            Ok(_) => {}
            Err(e)
                if e.as_service_error()
                    .is_some_and(|se| se.is_conditional_check_failed_exception()) => {}
            Err(e) => return Err(AppError::Internal(format!("dynamodb delete_item: {e}"))),
        }
    }
    Ok(())
}

async fn query_prefix(
    client: &DbClient,
    event_id: Uuid,
    prefix: String,
) -> AppResult<Vec<StoredRunner>> {
    let mut out = Vec::new();
    let mut start: Option<HashMap<String, AttributeValue>> = None;
    loop {
        let res = client
            .inner
            .query()
            .table_name(&client.table_name)
            .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
            .expression_attribute_values(":pk", AttributeValue::S(keys::event_pk(event_id)))
            .expression_attribute_values(":prefix", AttributeValue::S(prefix.clone()))
            .set_exclusive_start_key(start.take())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;
        out.extend(parse_items(res.items.unwrap_or_default())?);
        start = res.last_evaluated_key;
        if start.is_none() {
            break;
        }
    }
    Ok(out)
}

/// The team's rows as stored (with their actual keys) — input for
/// [`plan_update`].
pub async fn list_stored_for_team(
    client: &DbClient,
    event_id: Uuid,
    team_id: Uuid,
) -> AppResult<Vec<StoredRunner>> {
    query_prefix(
        client,
        event_id,
        keys::event_sk_runner_prefix_for_team(team_id),
    )
    .await
}

pub async fn list_for_team(
    client: &DbClient,
    event_id: Uuid,
    team_id: Uuid,
) -> AppResult<Vec<Runner>> {
    Ok(normalize(
        list_stored_for_team(client, event_id, team_id).await?,
    ))
}

pub async fn list_for_event(client: &DbClient, event_id: Uuid) -> AppResult<Vec<Runner>> {
    Ok(normalize(
        query_prefix(client, event_id, keys::SK_RUNNER_PREFIX.to_string()).await?,
    ))
}

fn parse_items(items: Vec<HashMap<String, AttributeValue>>) -> AppResult<Vec<StoredRunner>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let parsed: RunnerItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        out.push(parsed.into_stored());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    //! The planner is exercised against an in-memory model of the table
    //! that applies [`RunnerWrite`]s with DynamoDB's semantics: puts
    //! replace the item at their key, deletes check `id = :id`, and a
    //! transaction is all-or-nothing.

    use std::collections::BTreeMap;

    use chrono::TimeZone;

    use super::*;

    const EVENT: Uuid = Uuid::from_u128(0xe);
    const TEAM: Uuid = Uuid::from_u128(0x7);
    const OTHER_TEAM: Uuid = Uuid::from_u128(0x8);
    const A: Uuid = Uuid::from_u128(0xa);
    const B: Uuid = Uuid::from_u128(0xb);
    const C: Uuid = Uuid::from_u128(0xc);

    fn t(h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 19, h, 0, 0).unwrap()
    }

    fn runner(id: Uuid, team_id: Uuid, position: u32, last: &str) -> Runner {
        Runner {
            id,
            team_id,
            event_id: EVENT,
            first_name: "X".into(),
            last_name: last.into(),
            birth_date: NaiveDate::from_ymd_opt(2014, 1, 1).unwrap(),
            position,
            gender: None,
            created_at: t(8),
            updated_at: t(8),
        }
    }

    /// SK → runner, like one event partition.
    #[derive(Default, Clone)]
    struct Table(BTreeMap<String, Runner>);

    impl Table {
        fn put(&mut self, r: Runner) {
            self.0.insert(current_sk(&r), r);
        }
        fn put_legacy(&mut self, r: Runner) {
            self.0
                .insert(keys::event_sk_runner_legacy(r.team_id, r.position), r);
        }
        fn condition_ok(&self, w: &RunnerWrite) -> bool {
            match w {
                RunnerWrite::Put(_) => true,
                RunnerWrite::Delete { sk, expect_id } => {
                    self.0.get(sk).is_some_and(|r| r.id == *expect_id)
                }
            }
        }
        fn apply_one(&mut self, w: &RunnerWrite) {
            match w {
                RunnerWrite::Put(r) => self.put(r.clone()),
                RunnerWrite::Delete { sk, .. } => {
                    self.0.remove(sk);
                }
            }
        }
        /// `TransactWriteItems`: all conditions or nothing. Also enforces
        /// DynamoDB's "one operation per item" rule.
        fn transact(&mut self, writes: &[RunnerWrite]) -> Result<(), &'static str> {
            let mut touched = HashSet::new();
            for w in writes {
                let sk = match w {
                    RunnerWrite::Put(r) => current_sk(r),
                    RunnerWrite::Delete { sk, .. } => sk.clone(),
                };
                assert!(
                    touched.insert(sk),
                    "two operations on one item in a transaction"
                );
            }
            if !writes.iter().all(|w| self.condition_ok(w)) {
                return Err("TransactionCanceled");
            }
            for w in writes {
                self.apply_one(w);
            }
            Ok(())
        }
        /// `runners::delete`: each op on its own, failed condition ignored.
        fn delete_tolerant(&mut self, r: &Runner) {
            for w in delete_ops(r) {
                if self.condition_ok(&w) {
                    self.apply_one(&w);
                }
            }
        }
        fn stored(&self, team: Uuid) -> Vec<StoredRunner> {
            let prefix = keys::event_sk_runner_prefix_for_team(team);
            self.0
                .iter()
                .filter(|(sk, _)| sk.starts_with(&prefix))
                .map(|(sk, r)| StoredRunner {
                    sk: sk.clone(),
                    runner: r.clone(),
                })
                .collect()
        }
        fn list(&self, team: Uuid) -> Vec<Runner> {
            normalize(self.stored(team))
        }
        fn positions(&self, team: Uuid) -> Vec<(Uuid, u32)> {
            self.list(team).iter().map(|r| (r.id, r.position)).collect()
        }
        /// What the `update` route does.
        fn update_position(&mut self, id: Uuid, position: u32) -> Result<(), &'static str> {
            let stored = self.stored(TEAM);
            let mut updated = normalize(stored.clone())
                .into_iter()
                .find(|r| r.id == id)
                .expect("runner exists");
            updated.position = position;
            updated.updated_at = t(9);
            let writes = plan_update(&stored, &updated, t(9)).expect("plan");
            self.transact(&writes)
        }
    }

    fn team_ab() -> Table {
        let mut tbl = Table::default();
        tbl.put(runner(A, TEAM, 1, "Anton"));
        tbl.put(runner(B, TEAM, 2, "Berta"));
        tbl
    }

    #[test]
    fn move_to_occupied_position_swaps() {
        let mut tbl = team_ab();
        tbl.update_position(A, 2).unwrap();
        assert_eq!(tbl.positions(TEAM), vec![(B, 1), (A, 2)]);
        assert_eq!(tbl.0.len(), 2, "no leftover or duplicated rows");
        let b = tbl.list(TEAM).into_iter().find(|r| r.id == B).unwrap();
        assert_eq!(b.updated_at, t(9));
    }

    #[test]
    fn move_back_restores_original_order() {
        let mut tbl = team_ab();
        tbl.update_position(A, 2).unwrap();
        tbl.update_position(A, 1).unwrap();
        assert_eq!(tbl.positions(TEAM), vec![(A, 1), (B, 2)]);
        assert_eq!(tbl.0.len(), 2);
    }

    #[test]
    fn move_to_free_position_only_moves_the_runner() {
        let mut tbl = team_ab();
        tbl.update_position(A, 5).unwrap();
        assert_eq!(tbl.positions(TEAM), vec![(B, 2), (A, 5)]);
        assert_eq!(tbl.0.len(), 2);
    }

    #[test]
    fn swap_leaves_other_teams_alone() {
        let mut tbl = team_ab();
        tbl.put(runner(C, OTHER_TEAM, 2, "Cäsar"));
        tbl.update_position(A, 2).unwrap();
        assert_eq!(tbl.positions(OTHER_TEAM), vec![(C, 2)]);
        assert_eq!(tbl.positions(TEAM), vec![(B, 1), (A, 2)]);
    }

    #[test]
    fn swap_plan_is_two_deletes_two_puts() {
        let tbl = team_ab();
        let stored = tbl.stored(TEAM);
        let mut a = runner(A, TEAM, 2, "Anton");
        a.updated_at = t(9);
        let writes = plan_update(&stored, &a, t(9)).unwrap();
        let deletes = writes
            .iter()
            .filter(|w| matches!(w, RunnerWrite::Delete { .. }))
            .count();
        let puts = writes.len() - deletes;
        assert_eq!((deletes, puts), (2, 2));
    }

    #[test]
    fn name_change_without_move_is_a_single_put() {
        let tbl = team_ab();
        let mut a = runner(A, TEAM, 1, "Neuname");
        a.updated_at = t(9);
        let writes = plan_update(&tbl.stored(TEAM), &a, t(9)).unwrap();
        assert!(matches!(writes.as_slice(), [RunnerWrite::Put(r)] if r.last_name == "Neuname"));
    }

    #[test]
    fn update_unknown_runner_is_not_found() {
        let tbl = team_ab();
        let ghost = runner(C, TEAM, 1, "Geist");
        let err = plan_update(&tbl.stored(TEAM), &ghost, t(9)).unwrap_err();
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn create_at_occupied_position_keeps_both() {
        let mut tbl = team_ab();
        // `create` = plain put of a fresh runner at position 2.
        tbl.put(runner(C, TEAM, 2, "Aaron"));
        let list = tbl.list(TEAM);
        assert_eq!(list.len(), 3);
        // Same position → sorted by name.
        assert_eq!(
            list.iter().map(|r| (r.id, r.position)).collect::<Vec<_>>(),
            vec![(A, 1), (C, 2), (B, 2)]
        );
    }

    #[test]
    fn move_into_duplicate_position_swaps_all_holders() {
        let mut tbl = team_ab();
        tbl.put(runner(C, TEAM, 2, "Cäsar"));
        tbl.update_position(A, 2).unwrap();
        assert_eq!(tbl.positions(TEAM), vec![(B, 1), (C, 1), (A, 2)]);
        assert_eq!(tbl.0.len(), 3);
    }

    #[test]
    fn legacy_rows_are_listed_with_new_rows_sorted() {
        let mut tbl = Table::default();
        tbl.put_legacy(runner(B, TEAM, 2, "Berta"));
        tbl.put(runner(A, TEAM, 3, "Anton"));
        tbl.put_legacy(runner(C, TEAM, 1, "Cäsar"));
        assert_eq!(tbl.positions(TEAM), vec![(C, 1), (B, 2), (A, 3)]);
    }

    #[test]
    fn list_dedupes_runner_present_under_both_keys() {
        let mut tbl = Table::default();
        tbl.put_legacy(runner(A, TEAM, 1, "Alt"));
        tbl.put(runner(A, TEAM, 1, "Neu"));
        let list = tbl.list(TEAM);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].last_name, "Neu", "current-key row wins");
    }

    #[test]
    fn legacy_swap_moves_both_rows_to_new_keys() {
        let mut tbl = Table::default();
        tbl.put_legacy(runner(A, TEAM, 1, "Anton"));
        tbl.put_legacy(runner(B, TEAM, 2, "Berta"));
        tbl.update_position(A, 2).unwrap();
        assert_eq!(tbl.positions(TEAM), vec![(B, 1), (A, 2)]);
        assert_eq!(tbl.0.len(), 2);
        assert!(tbl.stored(TEAM).iter().all(StoredRunner::has_current_key));
    }

    #[test]
    fn legacy_row_is_rekeyed_on_plain_update() {
        let mut tbl = Table::default();
        tbl.put_legacy(runner(A, TEAM, 1, "Anton"));
        // Position unchanged → plan is delete legacy + put new.
        tbl.update_position(A, 1).unwrap();
        let stored = tbl.stored(TEAM);
        assert_eq!(stored.len(), 1);
        assert!(stored[0].has_current_key());
    }

    #[test]
    fn stale_roster_cancels_the_whole_swap() {
        let mut tbl = team_ab();
        let stored = tbl.stored(TEAM);
        // B is deleted by someone else between read and write.
        tbl.delete_tolerant(&runner(B, TEAM, 2, "Berta"));
        let mut a = runner(A, TEAM, 2, "Anton");
        a.updated_at = t(9);
        let writes = plan_update(&stored, &a, t(9)).unwrap();
        assert!(tbl.transact(&writes).is_err());
        // Nothing written: A still at 1, B not resurrected.
        assert_eq!(tbl.positions(TEAM), vec![(A, 1)]);
    }

    #[test]
    fn delete_removes_new_format_row() {
        let mut tbl = team_ab();
        tbl.delete_tolerant(&runner(A, TEAM, 1, "Anton"));
        assert_eq!(tbl.positions(TEAM), vec![(B, 2)]);
    }

    #[test]
    fn delete_removes_legacy_format_row() {
        let mut tbl = Table::default();
        tbl.put_legacy(runner(A, TEAM, 1, "Anton"));
        tbl.put_legacy(runner(B, TEAM, 2, "Berta"));
        tbl.delete_tolerant(&runner(A, TEAM, 1, "Anton"));
        assert_eq!(tbl.positions(TEAM), vec![(B, 2)]);
        assert_eq!(tbl.0.len(), 1);
    }

    #[test]
    fn legacy_delete_never_removes_another_runner() {
        // B sits under the legacy key of position 2; A (new key) claims
        // position 2 too. Deleting A must not touch B's legacy row.
        let mut tbl = Table::default();
        tbl.put_legacy(runner(B, TEAM, 2, "Berta"));
        tbl.put(runner(A, TEAM, 2, "Anton"));
        tbl.delete_tolerant(&runner(A, TEAM, 2, "Anton"));
        assert_eq!(tbl.positions(TEAM), vec![(B, 2)]);
    }

    #[test]
    fn delete_ops_are_conditional_on_the_runner_id() {
        let a = runner(A, TEAM, 3, "Anton");
        let sks: Vec<String> = delete_ops(&a)
            .iter()
            .map(|w| match w {
                RunnerWrite::Delete { sk, expect_id } => {
                    assert_eq!(*expect_id, A);
                    sk.clone()
                }
                RunnerWrite::Put(_) => panic!("delete_ops must only delete"),
            })
            .collect();
        assert_eq!(
            sks,
            vec![
                keys::event_sk_runner(TEAM, 3, A),
                keys::event_sk_runner_legacy(TEAM, 3)
            ]
        );
    }

    #[test]
    fn delete_of_missing_runner_is_a_noop() {
        let mut tbl = team_ab();
        tbl.delete_tolerant(&runner(C, TEAM, 2, "Cäsar"));
        assert_eq!(tbl.positions(TEAM), vec![(A, 1), (B, 2)]);
    }

    #[test]
    fn item_roundtrip_keeps_stored_key() {
        let a = runner(A, TEAM, 4, "Anton");
        let item = to_item(&a).unwrap();
        assert_eq!(
            item.get("SK").and_then(|v| v.as_s().ok()).cloned(),
            Some(keys::event_sk_runner(TEAM, 4, A))
        );
        let stored = parse_items(vec![item]).unwrap();
        assert!(stored[0].has_current_key());
        assert_eq!(stored[0].runner.position, 4);
    }

    #[test]
    fn legacy_item_parses_and_is_flagged_as_legacy() {
        let a = runner(A, TEAM, 4, "Anton");
        let mut item = to_item(&a).unwrap();
        item.insert(
            "SK".into(),
            AttributeValue::S(keys::event_sk_runner_legacy(TEAM, 4)),
        );
        let stored = parse_items(vec![item]).unwrap();
        assert!(!stored[0].has_current_key());
        assert_eq!(stored[0].runner.id, A);
    }
}
