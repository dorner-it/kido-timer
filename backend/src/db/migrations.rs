// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! One-shot data migrations for the single-table layout.
//!
//! These run **outside** the Lambda — they are tools the operator invokes
//! locally against an AWS profile (typically `--profile aws`). They must
//! never be wired into the request path: a Lambda cold start must not
//! scan the table.

use std::collections::HashMap;

use aws_sdk_dynamodb::types::{AttributeValue, Delete, Put, TransactWriteItem};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};

/// Map a v1 (`schema_version = 1` / pre-v3) timer mode string to its v3
/// discipline-specific equivalent.
///
/// Rationale:
/// - 1- and 2-lane variants (`single_lane`, `two_lane_parallel`, `relay`)
///   were all running-relay-style formats → `gruppenstaffette` (2 lanes).
/// - `individual` was the de-facto 4-lane parallel format used for
///   Löschangriff scoring → `loeschangriff` (4 lanes).
///
/// Returns `None` for already-migrated values and unknown strings (the
/// caller decides whether to log or fail).
pub fn map_legacy_timer_mode(old: &str) -> Option<&'static str> {
    match old {
        "single_lane" | "two_lane_parallel" | "relay" => Some("gruppenstaffette"),
        "individual" => Some("loeschangriff"),
        _ => None,
    }
}

/// Outcome of a single-table scan/update pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct MigrationReport {
    /// Items inspected (had any `mode` attribute).
    pub inspected: usize,
    /// Items rewritten to a v3 value.
    pub updated: usize,
    /// Items already on a v3 value — left untouched.
    pub already_v3: usize,
    /// Items with an unknown `mode` — left untouched. The operator should
    /// investigate these manually.
    pub unknown: usize,
}

/// Scan the table for items whose `mode` attribute is a v1 string and
/// rewrite them in place to v3. Idempotent: re-running after success is
/// a no-op.
///
/// Implementation note: this is intentionally a `Scan`, which is only
/// acceptable because it runs ad-hoc from a developer machine, not from
/// the Lambda hot path. The repo's "no scans in production code paths"
/// rule applies to the request handlers, not to one-off tooling.
pub async fn migrate_timer_mode_inplace(
    client: &DbClient,
    dry_run: bool,
) -> AppResult<MigrationReport> {
    let mut report = MigrationReport::default();
    let mut exclusive_start_key: Option<std::collections::HashMap<String, AttributeValue>> = None;

    loop {
        let mut req = client
            .inner
            .scan()
            .table_name(&client.table_name)
            .projection_expression("#pk, #sk, #mode")
            .expression_attribute_names("#pk", "PK")
            .expression_attribute_names("#sk", "SK")
            .expression_attribute_names("#mode", "mode")
            .filter_expression("attribute_exists(#mode)");

        if let Some(start) = exclusive_start_key.take() {
            req = req.set_exclusive_start_key(Some(start));
        }

        let page = req
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("scan: {e}")))?;

        for item in page.items.unwrap_or_default() {
            let Some(AttributeValue::S(mode_val)) = item.get("mode") else {
                continue;
            };
            report.inspected += 1;

            match mode_val.as_str() {
                "gruppenstaffette" | "loeschangriff" => {
                    report.already_v3 += 1;
                    continue;
                }
                other => {
                    let Some(new_mode) = map_legacy_timer_mode(other) else {
                        report.unknown += 1;
                        tracing::warn!(
                            pk = ?item.get("PK"),
                            sk = ?item.get("SK"),
                            mode = other,
                            "unknown timer mode, skipped"
                        );
                        continue;
                    };

                    let (Some(pk), Some(sk)) = (item.get("PK").cloned(), item.get("SK").cloned())
                    else {
                        return Err(AppError::Internal("scanned item missing PK/SK".into()));
                    };

                    if dry_run {
                        tracing::info!(
                            ?pk,
                            ?sk,
                            from = other,
                            to = new_mode,
                            "would update (dry run)"
                        );
                    } else {
                        client
                            .inner
                            .update_item()
                            .table_name(&client.table_name)
                            .key("PK", pk)
                            .key("SK", sk)
                            .update_expression("SET #mode = :new")
                            .expression_attribute_names("#mode", "mode")
                            .expression_attribute_values(
                                ":new",
                                AttributeValue::S(new_mode.to_string()),
                            )
                            .send()
                            .await
                            .map_err(|e| AppError::Internal(format!("update_item: {e}")))?;
                    }
                    report.updated += 1;
                }
            }
        }

        exclusive_start_key = page.last_evaluated_key;
        if exclusive_start_key.is_none() {
            break;
        }
    }

    Ok(report)
}

// ===== Runner sort keys: RUNNER#<team>#<pos> → RUNNER#<team>#<pos>#<id> =====

/// Where a scanned runner row's key stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerKeyState {
    /// Already `RUNNER#<team_id>#<position:04>#<id>`.
    Current,
    /// Legacy `RUNNER#<team_id>#<position:04>` — move to `new_sk`.
    Legacy { new_sk: String },
    /// Key matches neither shape for the row's own attributes — left alone.
    Unknown,
}

/// Classify a runner row by its `SK` and its `team_id` / `position` / `id`
/// attributes. Only the operator migration looks at key shapes; the request
/// path reads both via the `RUNNER#<team_id>#` prefix.
pub fn classify_runner_sk(sk: &str, team_id: Uuid, position: u32, id: Uuid) -> RunnerKeyState {
    let new_sk = keys::event_sk_runner(team_id, position, id);
    if sk == new_sk {
        RunnerKeyState::Current
    } else if sk == keys::event_sk_runner_legacy(team_id, position) {
        RunnerKeyState::Legacy { new_sk }
    } else {
        RunnerKeyState::Unknown
    }
}

/// Outcome of [`migrate_runner_keys`].
#[derive(Debug, Default, Clone, Copy)]
pub struct RunnerKeyMigrationReport {
    /// Runner rows inspected.
    pub inspected: usize,
    /// Legacy rows moved to the new key (or that would be, on dry run).
    pub migrated: usize,
    /// Rows already on the new key.
    pub already_current: usize,
    /// Legacy rows whose runner already had a new-key row: the stale
    /// legacy copy was removed (the new row is authoritative).
    pub stale_legacy_removed: usize,
    /// Rows with an unexpected key/attribute combination, or rows that
    /// changed while migrating — left untouched; check the logs.
    pub skipped: usize,
}

fn attr_s<'a>(item: &'a HashMap<String, AttributeValue>, name: &str) -> Option<&'a str> {
    item.get(name)
        .and_then(|v| v.as_s().ok())
        .map(String::as_str)
}

fn attr_uuid(item: &HashMap<String, AttributeValue>, name: &str) -> Option<Uuid> {
    attr_s(item, name).and_then(|s| Uuid::parse_str(s).ok())
}

fn attr_u32(item: &HashMap<String, AttributeValue>, name: &str) -> Option<u32> {
    item.get(name)
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse().ok())
}

/// Scan for runner rows still on the legacy key and move each one to
/// `RUNNER#<team_id>#<position:04>#<id>`.
///
/// Per row, one transaction: put the item under the new key
/// (`attribute_not_exists(SK)` — never overwrite) and delete the legacy
/// key (`id = :id` — never another runner). If the transaction is
/// cancelled because the new key already exists, the legacy copy is stale
/// and is deleted on its own (again conditional on the id).
///
/// Idempotent: a second run finds only `already_current` rows. Operator
/// tool only (Scan) — not for the Lambda request path. Not needed for
/// correctness: the backend reads both shapes and re-keys a row on its
/// next write.
pub async fn migrate_runner_keys(
    client: &DbClient,
    dry_run: bool,
) -> AppResult<RunnerKeyMigrationReport> {
    let mut report = RunnerKeyMigrationReport::default();
    let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

    loop {
        let mut req = client
            .inner
            .scan()
            .table_name(&client.table_name)
            .filter_expression("begins_with(#sk, :prefix)")
            .expression_attribute_names("#sk", "SK")
            .expression_attribute_values(
                ":prefix",
                AttributeValue::S(keys::SK_RUNNER_PREFIX.to_string()),
            );
        if let Some(start) = exclusive_start_key.take() {
            req = req.set_exclusive_start_key(Some(start));
        }
        let page = req
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("scan: {e}")))?;

        for item in page.items.unwrap_or_default() {
            report.inspected += 1;
            let (Some(pk), Some(sk), Some(team_id), Some(position), Some(id)) = (
                attr_s(&item, "PK").map(str::to_string),
                attr_s(&item, "SK").map(str::to_string),
                attr_uuid(&item, "team_id"),
                attr_u32(&item, "position"),
                attr_uuid(&item, "id"),
            ) else {
                report.skipped += 1;
                tracing::warn!(sk = ?item.get("SK"), "runner row missing key attributes, skipped");
                continue;
            };

            let new_sk = match classify_runner_sk(&sk, team_id, position, id) {
                RunnerKeyState::Current => {
                    report.already_current += 1;
                    continue;
                }
                RunnerKeyState::Unknown => {
                    report.skipped += 1;
                    tracing::warn!(%pk, %sk, %id, position, "unexpected runner key, skipped");
                    continue;
                }
                RunnerKeyState::Legacy { new_sk } => new_sk,
            };

            if dry_run {
                tracing::info!(%pk, from = %sk, to = %new_sk, "would re-key runner (dry run)");
                report.migrated += 1;
                continue;
            }

            let mut new_item = item.clone();
            new_item.insert("SK".into(), AttributeValue::S(new_sk.clone()));
            let id_val = AttributeValue::S(id.to_string());
            let put = Put::builder()
                .table_name(&client.table_name)
                .set_item(Some(new_item))
                .condition_expression("attribute_not_exists(SK)")
                .build()
                .map_err(|e| AppError::Internal(format!("transact put: {e}")))?;
            let delete = Delete::builder()
                .table_name(&client.table_name)
                .key("PK", AttributeValue::S(pk.clone()))
                .key("SK", AttributeValue::S(sk.clone()))
                .condition_expression("id = :id")
                .expression_attribute_values(":id", id_val.clone())
                .build()
                .map_err(|e| AppError::Internal(format!("transact delete: {e}")))?;
            let res = client
                .inner
                .transact_write_items()
                .transact_items(TransactWriteItem::builder().put(put).build())
                .transact_items(TransactWriteItem::builder().delete(delete).build())
                .send()
                .await;
            match res {
                Ok(_) => report.migrated += 1,
                Err(e)
                    if e.as_service_error()
                        .is_some_and(|se| se.is_transaction_canceled_exception()) =>
                {
                    // New key already present → drop the stale legacy copy.
                    let del = client
                        .inner
                        .delete_item()
                        .table_name(&client.table_name)
                        .key("PK", AttributeValue::S(pk.clone()))
                        .key("SK", AttributeValue::S(sk.clone()))
                        .condition_expression("id = :id")
                        .expression_attribute_values(":id", id_val)
                        .send()
                        .await;
                    match del {
                        Ok(_) => {
                            report.stale_legacy_removed += 1;
                            tracing::warn!(%pk, %sk, %new_sk, "new key existed; removed stale legacy row");
                        }
                        Err(e)
                            if e.as_service_error()
                                .is_some_and(|se| se.is_conditional_check_failed_exception()) =>
                        {
                            report.skipped += 1;
                            tracing::warn!(%pk, %sk, "legacy row changed during migration, skipped");
                        }
                        Err(e) => return Err(AppError::Internal(format!("delete_item: {e}"))),
                    }
                }
                Err(e) => return Err(AppError::Internal(format!("transact_write_items: {e}"))),
            }
        }

        exclusive_start_key = page.last_evaluated_key;
        if exclusive_start_key.is_none() {
            break;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_mapping_covers_all_v1_values() {
        assert_eq!(
            map_legacy_timer_mode("single_lane"),
            Some("gruppenstaffette")
        );
        assert_eq!(
            map_legacy_timer_mode("two_lane_parallel"),
            Some("gruppenstaffette")
        );
        assert_eq!(map_legacy_timer_mode("relay"), Some("gruppenstaffette"));
        assert_eq!(map_legacy_timer_mode("individual"), Some("loeschangriff"));
    }

    #[test]
    fn legacy_mapping_returns_none_for_v3_and_garbage() {
        assert_eq!(map_legacy_timer_mode("gruppenstaffette"), None);
        assert_eq!(map_legacy_timer_mode("loeschangriff"), None);
        assert_eq!(map_legacy_timer_mode("unknown_mode"), None);
        assert_eq!(map_legacy_timer_mode(""), None);
    }

    #[test]
    fn runner_key_classification() {
        let team = Uuid::from_u128(1);
        let id = Uuid::from_u128(2);
        assert_eq!(
            classify_runner_sk(&keys::event_sk_runner(team, 3, id), team, 3, id),
            RunnerKeyState::Current
        );
        assert_eq!(
            classify_runner_sk(&keys::event_sk_runner_legacy(team, 3), team, 3, id),
            RunnerKeyState::Legacy {
                new_sk: keys::event_sk_runner(team, 3, id)
            }
        );
        // Position attribute disagrees with the key → don't guess.
        assert_eq!(
            classify_runner_sk(&keys::event_sk_runner_legacy(team, 3), team, 4, id),
            RunnerKeyState::Unknown
        );
        // Another runner's new key.
        assert_eq!(
            classify_runner_sk(
                &keys::event_sk_runner(team, 3, Uuid::from_u128(9)),
                team,
                3,
                id
            ),
            RunnerKeyState::Unknown
        );
    }

    #[test]
    fn runner_key_attrs_parse_from_item() {
        let mut item = HashMap::new();
        item.insert(
            "id".to_string(),
            AttributeValue::S(Uuid::from_u128(5).to_string()),
        );
        item.insert("position".to_string(), AttributeValue::N("12".into()));
        assert_eq!(attr_uuid(&item, "id"), Some(Uuid::from_u128(5)));
        assert_eq!(attr_u32(&item, "position"), Some(12));
        assert_eq!(attr_u32(&item, "missing"), None);
    }
}
