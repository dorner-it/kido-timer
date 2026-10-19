// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Pure merge / validation logic for the on-prem round trip
//! (`docs/contract-pr6-onprem.md`, section A):
//!
//! - `POST /api/export/disciplines/{id}/onprem-upload` ([`plan_upload`]),
//! - the `penalties` array of a `.kido` import ([`normalize_penalty`] +
//!   [`merge_review`]).
//!
//! No I/O here: the handlers load the discipline's rows, call these
//! functions and write whatever the returned plan says. That keeps the
//! rules (idempotency, status precedence, catalogue recomputation,
//! ownership checks) unit-testable without DynamoDB.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::penalty::catalogue_delta_ms;
use crate::models::{
    Discipline, LineupConfirmedBy, Penalty, PenaltyKind, PenaltyStatus, Run, RunStatus, Runner,
    TimeSource,
};
use crate::routes::decider::validate_station_submission;
use crate::routes::events::{SetCurrentInput, resolve_current_run_ids};
use crate::routes::runs::validate_lineup;

/// `reviewed_by` written when an upload/import marks a penalty reviewed
/// without saying who did it.
pub const ONPREM_REVIEWER: &str = "onprem-upload";

// ===== wire types =====

/// Body of `POST /api/export/disciplines/{id}/onprem-upload`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OnpremUploadInput {
    /// Only the runs the desktop changed.
    #[serde(default)]
    pub runs: Vec<UploadRun>,
    /// Full penalty rows (all statuses); `id` = client id from the judge
    /// phone.
    #[serde(default)]
    pub penalties: Vec<Penalty>,
    /// Current runs after the on-prem day. Omitted and `[]` both clear
    /// the event's current runs (when this discipline is the current
    /// one).
    #[serde(default)]
    pub current_run_ids: Option<Vec<Uuid>>,
}

/// The run fields the desktop may change on-prem. Everything else on the
/// stored run (team, lane, created_at, …) is left untouched.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadRun {
    pub id: Uuid,
    pub run_number: u32,
    pub status: RunStatus,
    #[serde(default)]
    pub original_time_ms: Option<u32>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub lineup: Vec<Uuid>,
    #[serde(default)]
    pub lineup_confirmed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub lineup_confirmed_by: Option<LineupConfirmedBy>,
    #[serde(default)]
    pub time_source: TimeSource,
    #[serde(default)]
    pub hand_time_1_ms: Option<u32>,
    #[serde(default)]
    pub hand_time_2_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnpremUploadResponse {
    /// Runs in the request that were applied (incl. unchanged ones).
    pub runs_updated: usize,
    /// Penalties in the request that were upserted (incl. unchanged ones).
    pub penalties_upserted: usize,
}

// ===== status precedence =====

/// `pending` < `approved` = `rejected`: a reviewed state is "more
/// advanced" than pending.
pub fn status_rank(s: PenaltyStatus) -> u8 {
    match s {
        PenaltyStatus::Pending => 0,
        PenaltyStatus::Approved | PenaltyStatus::Rejected => 1,
    }
}

/// Review fields after merging an incoming row onto the stored one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewMerge {
    pub status: PenaltyStatus,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
    /// Stored and incoming are reviewed with *different* outcomes
    /// (approved vs rejected); the stored value was kept. Callers log it.
    pub conflict: bool,
}

/// "Most advanced status wins":
///
/// - no stored row → the incoming review fields (a reviewed row without
///   `reviewed_at` / `reviewed_by` gets `now` / [`ONPREM_REVIEWER`]);
/// - incoming reviewed, stored pending → incoming wins (same filling);
/// - incoming pending, stored reviewed → stored wins;
/// - both pending → pending, no review fields;
/// - both reviewed → the stored row is kept verbatim (the server already
///   has a reviewed value); `conflict` is set when the outcomes differ.
///
/// Idempotent: merging the result with the same incoming row again
/// yields the result unchanged.
pub fn merge_review(
    existing: Option<&Penalty>,
    incoming: &Penalty,
    now: DateTime<Utc>,
) -> ReviewMerge {
    let take_incoming = || {
        if incoming.status == PenaltyStatus::Pending {
            ReviewMerge {
                status: PenaltyStatus::Pending,
                reviewed_at: None,
                reviewed_by: None,
                conflict: false,
            }
        } else {
            ReviewMerge {
                status: incoming.status,
                reviewed_at: Some(incoming.reviewed_at.unwrap_or(now)),
                reviewed_by: Some(
                    incoming
                        .reviewed_by
                        .clone()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| ONPREM_REVIEWER.to_string()),
                ),
                conflict: false,
            }
        }
    };
    let Some(existing) = existing else {
        return take_incoming();
    };
    let keep_existing = |conflict| ReviewMerge {
        status: existing.status,
        reviewed_at: existing.reviewed_at,
        reviewed_by: existing.reviewed_by.clone(),
        conflict,
    };
    match (status_rank(existing.status), status_rank(incoming.status)) {
        // Stored pending: incoming wins (reviewed or still pending).
        (0, _) => take_incoming(),
        (_, 0) => keep_existing(false),
        _ => keep_existing(existing.status != incoming.status),
    }
}

// ===== penalty normalization =====

/// What a penalty row is checked against.
pub struct DisciplineContext<'a> {
    pub discipline: &'a Discipline,
    /// Runs of the discipline by id.
    pub runs: &'a HashMap<Uuid, Run>,
    /// Stored penalties of the **whole event** by id (an id stored under
    /// another discipline is a foreign penalty).
    pub stored_penalties: &'a HashMap<Uuid, Penalty>,
}

/// Validate one incoming penalty and build the row to store.
///
/// - `event_id` / `discipline_id` must be this discipline's; the id must
///   not belong to a penalty of another discipline;
/// - `run_id` must be a run of this discipline (and, for a stored row,
///   the same run as before); `team_id` / `runner_id` are taken from the
///   run;
/// - catalogue row (`code` set): code must exist and belong to `station`
///   (a missing `station` is taken from the catalogue), the station must
///   be valid for the discipline mode; `kind`, `delta_ms` and
///   `is_disqualification` are recomputed (client values ignored);
/// - legacy row (`code` missing): only accepted when the row is already
///   stored — its content (kind, delta_ms, station, …) is kept, only the
///   review state merges;
/// - review state: [`merge_review`].
///
/// Returns the row and the [`ReviewMerge::conflict`] flag.
pub fn normalize_penalty(
    incoming: &Penalty,
    ctx: &DisciplineContext<'_>,
    now: DateTime<Utc>,
) -> AppResult<(Penalty, bool)> {
    let disc = ctx.discipline;
    if incoming.event_id != disc.event_id || incoming.discipline_id != disc.id {
        return Err(AppError::Validation(format!(
            "Fehler {} gehört nicht zu diesem Wettkampf",
            incoming.id
        )));
    }
    let stored = ctx.stored_penalties.get(&incoming.id);
    if stored.is_some_and(|s| s.discipline_id != disc.id || s.event_id != disc.event_id) {
        return Err(AppError::Validation(format!(
            "Fehler {} gehört zu einem anderen Wettkampf",
            incoming.id
        )));
    }
    let run = ctx.runs.get(&incoming.run_id).ok_or_else(|| {
        AppError::Validation(format!(
            "Fehler {}: Lauf {} gehört nicht zu diesem Wettkampf",
            incoming.id, incoming.run_id
        ))
    })?;
    if stored.is_some_and(|s| s.run_id != incoming.run_id) {
        return Err(AppError::Validation(format!(
            "Fehler {} gehört zu einem anderen Lauf",
            incoming.id
        )));
    }

    let review = merge_review(stored, incoming, now);

    let mut row = match (&incoming.code, stored) {
        (Some(code), _) => {
            let station = match incoming.station {
                Some(s) => s,
                None => {
                    kido_core::catalogue::lookup(code)
                        .ok_or_else(|| {
                            AppError::Validation(format!("unbekannter Fehlercode {code}"))
                        })?
                        .station
                }
            };
            let entry = validate_station_submission(disc.mode, station, code)?;
            Penalty {
                id: incoming.id,
                event_id: disc.event_id,
                discipline_id: disc.id,
                run_id: run.id,
                team_id: run.team_id,
                runner_id: run.runner_id,
                kind: entry.kind,
                code: Some(entry.code.to_string()),
                station: Some(station),
                is_disqualification: entry.kind == PenaltyKind::Fv,
                decider_slot: stored.map(|s| s.decider_slot).unwrap_or(0),
                delta_ms: catalogue_delta_ms(entry.kind),
                reason_code: stored.and_then(|s| s.reason_code.clone()),
                status: review.status,
                created_at: stored.map(|s| s.created_at).unwrap_or(incoming.created_at),
                reviewed_at: None,
                reviewed_by: None,
            }
        }
        (None, Some(s)) => s.clone(),
        (None, None) => {
            return Err(AppError::Validation(format!(
                "Fehler {}: Fehlercode fehlt",
                incoming.id
            )));
        }
    };
    row.status = review.status;
    row.reviewed_at = review.reviewed_at;
    row.reviewed_by = review.reviewed_by;
    Ok((row, review.conflict))
}

// ===== runs =====

/// Validate one uploaded run against the stored row and return the
/// updated row (`updated_at` is only bumped when a field changed, so a
/// repeated upload is a no-op). `roster` = the event's runners.
///
/// - `run_number` must match the stored run;
/// - lineup: ids belong to the run's team, no duplicates; an empty
///   lineup clears the confirmation fields;
/// - `handstopp`: both hand times > 0, `original_time_ms` (if sent) must
///   equal `hand_time_mean` (it is stored as that mean);
///   `lichtschranke`: no hand times.
pub fn apply_run_update(
    stored: &Run,
    upload: &UploadRun,
    roster: &[Runner],
    now: DateTime<Utc>,
) -> AppResult<Run> {
    if stored.run_number != upload.run_number {
        return Err(AppError::Validation(format!(
            "Lauf {}: run_number stimmt nicht mit dem gespeicherten Lauf überein",
            upload.id
        )));
    }
    validate_lineup(&upload.lineup, roster, stored.team_id)?;

    let original_time_ms = match upload.time_source {
        TimeSource::Handstopp => {
            let (Some(t1), Some(t2)) = (upload.hand_time_1_ms, upload.hand_time_2_ms) else {
                return Err(AppError::Validation(format!(
                    "Lauf {}: Handstopp braucht hand_time_1_ms und hand_time_2_ms",
                    upload.id
                )));
            };
            if t1 == 0 || t2 == 0 {
                return Err(AppError::Validation(format!(
                    "Lauf {}: Handzeiten müssen > 0 sein",
                    upload.id
                )));
            }
            let mean = kido_core::timing::hand_time_mean(t1, t2);
            if upload.original_time_ms.is_some_and(|t| t != mean) {
                return Err(AppError::Validation(format!(
                    "Lauf {}: original_time_ms muss der Mittelwert der Handzeiten sein ({mean})",
                    upload.id
                )));
            }
            Some(mean)
        }
        TimeSource::Lichtschranke => {
            if upload.hand_time_1_ms.is_some() || upload.hand_time_2_ms.is_some() {
                return Err(AppError::Validation(format!(
                    "Lauf {}: Handzeiten nur mit time_source = handstopp",
                    upload.id
                )));
            }
            upload.original_time_ms
        }
    };

    let (confirmed_at, confirmed_by) = if upload.lineup.is_empty() {
        (None, None)
    } else {
        (upload.lineup_confirmed_at, upload.lineup_confirmed_by)
    };

    let mut run = stored.clone();
    run.status = upload.status;
    run.original_time_ms = original_time_ms;
    run.started_at = upload.started_at;
    run.ended_at = upload.ended_at;
    run.lineup = upload.lineup.clone();
    run.lineup_confirmed_at = confirmed_at;
    run.lineup_confirmed_by = confirmed_by;
    run.time_source = upload.time_source;
    run.hand_time_1_ms = upload.hand_time_1_ms;
    run.hand_time_2_ms = upload.hand_time_2_ms;
    if run != *stored {
        run.updated_at = now;
    }
    Ok(run)
}

/// Legacy `Run.correction_ms`: sum of the approved penalties' `delta_ms`
/// (same rule as the organizer review routes). Display value for
/// pre-2027 clients only — scoring never reads it.
pub fn legacy_correction_ms<'a>(
    run_id: Uuid,
    penalties: impl IntoIterator<Item = &'a Penalty>,
) -> i32 {
    penalties
        .into_iter()
        .filter(|p| p.run_id == run_id && p.status == PenaltyStatus::Approved)
        .map(|p| p.delta_ms)
        .sum()
}

// ===== whole upload =====

/// What the upload handler writes.
#[derive(Debug, Clone, PartialEq)]
pub struct UploadPlan {
    /// Changed run rows (fields + recomputed `correction_ms`).
    pub runs_to_put: Vec<Run>,
    /// Changed penalty rows.
    pub penalties_to_put: Vec<Penalty>,
    /// `Some(ids)` → write the event's current runs (discipline stays /
    /// becomes this one). `None` → leave the event untouched.
    pub current_run_ids: Option<Vec<Uuid>>,
    /// Penalty ids where stored and uploaded reviews disagree (approved
    /// vs rejected); the stored value was kept.
    pub review_conflicts: Vec<Uuid>,
    pub response: OnpremUploadResponse,
}

/// Validate the whole upload (all-or-nothing: any invalid row → `400`
/// before anything is written) and compute the rows to write.
///
/// - `runs`: rows of this discipline; `penalties`: rows of the whole
///   event (to detect foreign ids); `roster`: runners of the event;
/// - `event_current`: the event's `(current_discipline_id,
///   current_run_ids)`.
///
/// `current_run_ids`: non-empty → validated like the organizer route
/// (runs of this discipline, ≤ `lane_count`, no duplicates) and set with
/// this discipline as the current one; omitted / `[]` → cleared, but
/// only when this discipline is the event's current one (another
/// discipline's state is never touched). Unchanged → `None`.
pub fn plan_upload(
    discipline: &Discipline,
    runs: &[Run],
    penalties: &[Penalty],
    roster: &[Runner],
    event_current: (Option<Uuid>, &[Uuid]),
    input: &OnpremUploadInput,
    now: DateTime<Utc>,
) -> AppResult<UploadPlan> {
    let runs_by_id: HashMap<Uuid, Run> = runs
        .iter()
        .filter(|r| r.discipline_id == discipline.id)
        .map(|r| (r.id, r.clone()))
        .collect();
    let stored_penalties: HashMap<Uuid, Penalty> =
        penalties.iter().map(|p| (p.id, p.clone())).collect();

    // --- runs ---
    let mut seen = HashSet::new();
    let mut updated_runs: HashMap<Uuid, Run> = HashMap::new();
    for up in &input.runs {
        if !seen.insert(up.id) {
            return Err(AppError::Validation(format!(
                "Lauf {} ist doppelt im Upload",
                up.id
            )));
        }
        let stored = runs_by_id.get(&up.id).ok_or_else(|| {
            AppError::Validation(format!("Lauf {} gehört nicht zu diesem Wettkampf", up.id))
        })?;
        updated_runs.insert(up.id, apply_run_update(stored, up, roster, now)?);
    }

    // --- penalties ---
    let ctx = DisciplineContext {
        discipline,
        runs: &runs_by_id,
        stored_penalties: &stored_penalties,
    };
    let mut seen = HashSet::new();
    let mut merged: Vec<Penalty> = Vec::with_capacity(input.penalties.len());
    let mut review_conflicts = Vec::new();
    for p in &input.penalties {
        if !seen.insert(p.id) {
            return Err(AppError::Validation(format!(
                "Fehler {} ist doppelt im Upload",
                p.id
            )));
        }
        let (row, conflict) = normalize_penalty(p, &ctx, now)?;
        if conflict {
            review_conflicts.push(row.id);
        }
        merged.push(row);
    }

    // --- current runs ---
    let (current_disc, current_ids) = event_current;
    let requested = input.current_run_ids.clone().unwrap_or_default();
    let discipline_run_ids: Vec<Uuid> = runs_by_id.keys().copied().collect();
    let resolved = resolve_current_run_ids(
        &SetCurrentInput {
            discipline_id: Some(discipline.id),
            run_id: None,
            run_ids: Some(requested),
        },
        Some(discipline.lane_count),
        &discipline_run_ids,
    )?;
    let current_run_ids = if resolved.is_empty() {
        (current_disc == Some(discipline.id) && !current_ids.is_empty()).then(Vec::new)
    } else {
        (current_disc != Some(discipline.id) || current_ids != resolved.as_slice())
            .then_some(resolved)
    };

    // --- final state: penalties to write, legacy correction per run ---
    let mut final_penalties: HashMap<Uuid, &Penalty> = stored_penalties
        .values()
        .filter(|p| p.discipline_id == discipline.id)
        .map(|p| (p.id, p))
        .collect();
    for p in &merged {
        final_penalties.insert(p.id, p);
    }
    let penalties_to_put: Vec<Penalty> = merged
        .iter()
        .filter(|p| stored_penalties.get(&p.id) != Some(*p))
        .cloned()
        .collect();

    let mut affected: Vec<Uuid> = input.runs.iter().map(|r| r.id).collect();
    for p in &merged {
        if !affected.contains(&p.run_id) {
            affected.push(p.run_id);
        }
    }
    let mut runs_to_put = Vec::new();
    for run_id in affected {
        let stored = &runs_by_id[&run_id];
        let mut run = updated_runs
            .remove(&run_id)
            .unwrap_or_else(|| stored.clone());
        let correction = legacy_correction_ms(run_id, final_penalties.values().copied());
        if run.correction_ms != correction {
            run.correction_ms = correction;
            run.updated_at = now;
        }
        if run != *stored {
            runs_to_put.push(run);
        }
    }

    Ok(UploadPlan {
        runs_to_put,
        penalties_to_put,
        current_run_ids,
        review_conflicts,
        response: OnpremUploadResponse {
            runs_updated: input.runs.len(),
            penalties_upserted: merged.len(),
        },
    })
}

// ===== .kido import (A1) =====

/// Result of merging a `.kido` file's `penalties` array.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPenaltiesPlan {
    /// Changed penalty rows to write.
    pub to_put: Vec<Penalty>,
    /// Rows accepted (written or already identical).
    pub imported: usize,
    /// Rows skipped (foreign discipline/run, unknown code, …).
    pub skipped: usize,
    /// Runs whose legacy `correction_ms` must be recomputed, with the
    /// new value.
    pub corrections: Vec<(Uuid, i32)>,
    pub review_conflicts: Vec<Uuid>,
}

/// Merge the `penalties` of an imported `.kido` file. Same rules as the
/// upload ([`normalize_penalty`]), but lenient like the rest of the
/// import: an invalid row is skipped (counted) instead of failing the
/// whole file. `runs` = the discipline's runs **after** the file's runs
/// were applied; `penalties` = the event's stored penalties.
pub fn plan_import_penalties(
    discipline: &Discipline,
    runs: &[Run],
    penalties: &[Penalty],
    incoming: &[Penalty],
    now: DateTime<Utc>,
) -> ImportPenaltiesPlan {
    let runs_by_id: HashMap<Uuid, Run> = runs
        .iter()
        .filter(|r| r.discipline_id == discipline.id)
        .map(|r| (r.id, r.clone()))
        .collect();
    let mut stored: HashMap<Uuid, Penalty> = penalties.iter().map(|p| (p.id, p.clone())).collect();

    let mut to_put: Vec<Penalty> = Vec::new();
    let mut imported = 0;
    let mut skipped = 0;
    let mut review_conflicts = Vec::new();
    let mut touched_runs: Vec<Uuid> = Vec::new();
    for p in incoming {
        let ctx = DisciplineContext {
            discipline,
            runs: &runs_by_id,
            stored_penalties: &stored,
        };
        match normalize_penalty(p, &ctx, now) {
            Ok((row, conflict)) => {
                imported += 1;
                if conflict {
                    review_conflicts.push(row.id);
                }
                if !touched_runs.contains(&row.run_id) {
                    touched_runs.push(row.run_id);
                }
                if stored.get(&row.id) != Some(&row) {
                    to_put.retain(|q| q.id != row.id);
                    to_put.push(row.clone());
                    stored.insert(row.id, row);
                }
            }
            Err(e) => {
                tracing::warn!(penalty_id = %p.id, error = %e, "import: penalty skipped");
                skipped += 1;
            }
        }
    }

    let corrections = touched_runs
        .into_iter()
        .filter_map(|run_id| {
            let value = legacy_correction_ms(
                run_id,
                stored.values().filter(|p| p.discipline_id == discipline.id),
            );
            (runs_by_id.get(&run_id)?.correction_ms != value).then_some((run_id, value))
        })
        .collect();

    ImportPenaltiesPlan {
        to_put,
        imported,
        skipped,
        corrections,
        review_conflicts,
    }
}

#[cfg(test)]
mod tests;
