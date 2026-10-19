// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! One-shot data migrations for the single-table layout.
//!
//! These run **outside** the Lambda — they are tools the operator invokes
//! locally against an AWS profile (typically `--profile aws`). They must
//! never be wired into the request path: a Lambda cold start must not
//! scan the table.

use aws_sdk_dynamodb::types::AttributeValue;

use crate::db::client::DbClient;
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
}
