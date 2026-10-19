// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{penalties, runs};
use crate::error::{AppError, AppResult};
use crate::models::{Penalty, PenaltyKind, PenaltyStatus, Run};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/penalties", get(list))
        .route("/{id}/penalties/{penalty_id}/approve", post(approve))
        .route("/{id}/penalties/{penalty_id}/reject", post(reject))
        .route("/{id}/penalties/{penalty_id}/undo", post(undo))
        .route(
            "/{id}/runs/{run_id}/penalties/bulk-approve",
            post(bulk_approve),
        )
}

#[derive(Deserialize)]
pub struct PenaltyQuery {
    pub status: Option<PenaltyStatus>,
    pub discipline_id: Option<Uuid>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(event_id): Path<Uuid>,
    Query(q): Query<PenaltyQuery>,
) -> AppResult<Json<Vec<Penalty>>> {
    require_owned(&state, &auth, event_id).await?;
    let mut result = match q.status {
        Some(status) => penalties::list_by_status(&state.db, event_id, status).await?,
        None => penalties::list_for_event(&state.db, event_id).await?,
    };
    if let Some(did) = q.discipline_id {
        result.retain(|p| p.discipline_id == did);
    }
    Ok(Json(result))
}

pub async fn approve(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, penalty_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Penalty>> {
    require_owned(&state, &auth, event_id).await?;
    let mut penalty = find_penalty(&state, event_id, penalty_id).await?;

    decide(
        &mut penalty,
        PenaltyStatus::Approved,
        Utc::now(),
        &auth.user.sub,
    )?;
    penalties::put(&state.db, &penalty).await?;

    recompute_run_correction(&state, event_id, penalty.discipline_id, penalty.run_id).await?;

    Ok(Json(penalty))
}

pub async fn reject(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, penalty_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Penalty>> {
    require_owned(&state, &auth, event_id).await?;
    let mut penalty = find_penalty(&state, event_id, penalty_id).await?;

    decide(
        &mut penalty,
        PenaltyStatus::Rejected,
        Utc::now(),
        &auth.user.sub,
    )?;
    penalties::put(&state.db, &penalty).await?;

    Ok(Json(penalty))
}

/// "Rückgängig": an individually decided penalty (approved or rejected)
/// goes back to `pending` until the run's "Übernehmen" froze it
/// ([`kido_core::penalty::can_undo_review`]). Clears `reviewed_*` and
/// recomputes the legacy `correction_ms`.
pub async fn undo(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, penalty_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Penalty>> {
    require_owned(&state, &auth, event_id).await?;
    let mut penalty = find_penalty(&state, event_id, penalty_id).await?;
    let run = runs::find(&state.db, event_id, penalty.discipline_id, penalty.run_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("run {}", penalty.run_id)))?;

    undo_review(&mut penalty, &run)?;
    penalties::put(&state.db, &penalty).await?;

    recompute_run_correction(&state, event_id, penalty.discipline_id, penalty.run_id).await?;

    Ok(Json(penalty))
}

// ===== pure review transitions (unit-tested without DynamoDB) =====

/// "Bestätigen" / "Zurücknehmen": only a pending penalty can be decided.
pub fn decide(
    penalty: &mut Penalty,
    status: PenaltyStatus,
    now: DateTime<Utc>,
    reviewer: &str,
) -> AppResult<()> {
    if penalty.status != PenaltyStatus::Pending {
        return Err(AppError::Conflict(format!(
            "penalty already {}",
            penalty.status.as_str()
        )));
    }
    penalty.status = status;
    penalty.reviewed_at = Some(now);
    penalty.reviewed_by = Some(reviewer.to_string());
    Ok(())
}

/// "Rückgängig": decided → pending, unless the run's "Übernehmen" froze
/// the decision (`409 conflict`).
pub fn undo_review(penalty: &mut Penalty, run: &Run) -> AppResult<()> {
    if penalty.status == PenaltyStatus::Pending {
        return Err(AppError::Conflict(
            "Fehler ist noch offen – nichts rückgängig zu machen".into(),
        ));
    }
    if !kido_core::penalty::can_undo_review(
        penalty.status,
        penalty.reviewed_at,
        run.penalties_accepted_at,
    ) {
        return Err(AppError::Conflict(
            "Lauf wurde bereits übernommen – die Entscheidung kann nicht mehr rückgängig gemacht werden".into(),
        ));
    }
    penalty.status = PenaltyStatus::Pending;
    penalty.reviewed_at = None;
    penalty.reviewed_by = None;
    Ok(())
}

/// "Übernehmen" for one run: every pending penalty of `run` → approved
/// with `reviewed_at = now`, and `run.penalties_accepted_at = now` (so the
/// strict `>` of the undo rule freezes them and everything decided
/// before). Returns the changed penalties.
pub fn accept_run(
    run: &mut Run,
    penalties: &[Penalty],
    now: DateTime<Utc>,
    reviewer: &str,
) -> Vec<Penalty> {
    run.penalties_accepted_at = Some(now);
    penalties
        .iter()
        .filter(|p| p.run_id == run.id && p.status == PenaltyStatus::Pending)
        .map(|p| {
            let mut p = p.clone();
            p.status = PenaltyStatus::Approved;
            p.reviewed_at = Some(now);
            p.reviewed_by = Some(reviewer.to_string());
            p
        })
        .collect()
}

async fn find_penalty(state: &AppState, event_id: Uuid, penalty_id: Uuid) -> AppResult<Penalty> {
    penalties::list_for_event(&state.db, event_id)
        .await?
        .into_iter()
        .find(|p| p.id == penalty_id)
        .ok_or_else(|| AppError::NotFound(format!("penalty {penalty_id}")))
}

// ===== bulk-approve =====
//
// "Lauf abschließen" / "Übernehmen": approve every pending penalty for
// one run in a single click and record `Run.penalties_accepted_at` (every
// time it is pressed). That timestamp freezes all decisions made so far:
// "Rückgängig" only works for decisions made after it. We deliberately
// do NOT auto-reject anything — only the organiser decides what to throw
// away. The response keeps a `rejected` counter for the response shape's
// stability (always 0 today; reserved for a future "approve good / reject
// the rest" combined action).

#[derive(Debug, Serialize)]
pub struct BulkApproveResponse {
    pub approved: u32,
    pub rejected: u32,
    /// Approved FV ("Fehlversuch") penalties. Name kept for old clients.
    pub dq: u32,
    /// The run's new `penalties_accepted_at` (server clock). Additive.
    pub penalties_accepted_at: DateTime<Utc>,
}

pub async fn bulk_approve(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<BulkApproveResponse>> {
    require_owned(&state, &auth, event_id).await?;

    let mut run = runs::list_for_event(&state.db, event_id)
        .await?
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;
    let all = penalties::list_for_event(&state.db, event_id).await?;

    let now = Utc::now();
    let changed = accept_run(&mut run, &all, now, &auth.user.sub);
    let mut dq: u32 = 0;
    for p in &changed {
        if p.kind == PenaltyKind::Fv {
            dq += 1;
        }
        penalties::put(&state.db, p).await?;
    }

    runs::update_penalties_accepted_at(
        &state.db,
        event_id,
        run.discipline_id,
        run.run_number,
        run.id,
        now,
    )
    .await?;
    if !changed.is_empty() {
        recompute_run_correction(&state, event_id, run.discipline_id, run.id).await?;
    }

    Ok(Json(BulkApproveResponse {
        approved: changed.len() as u32,
        rejected: 0,
        dq,
        penalties_accepted_at: now,
    }))
}

/// Writes the sum of approved `delta_ms` into `Run.correction_ms`.
/// Legacy display value for pre-2027 clients only — scoring
/// (`kido_core::scoring`) never reads `correction_ms` and adds 15 s per
/// counting SZ itself, so penalties are not counted twice
/// (see docs/contract-pr1-2027.md).
async fn recompute_run_correction(
    state: &AppState,
    event_id: Uuid,
    discipline_id: Uuid,
    run_id: Uuid,
) -> AppResult<()> {
    let approved = penalties::list_by_status(&state.db, event_id, PenaltyStatus::Approved).await?;
    let total: i32 = approved
        .iter()
        .filter(|p| p.run_id == run_id)
        .map(|p| p.delta_ms)
        .sum();

    let runs_for_disc = runs::list_for_discipline(&state.db, event_id, discipline_id).await?;
    let target = runs_for_disc
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;

    runs::update_correction_ms(
        &state.db,
        event_id,
        discipline_id,
        target.run_number,
        run_id,
        total,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RunStatus, TimeSource};
    use crate::onprem::legacy_correction_ms;

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn run() -> Run {
        Run {
            id: Uuid::new_v4(),
            event_id: Uuid::nil(),
            discipline_id: Uuid::nil(),
            team_id: Uuid::nil(),
            runner_id: None,
            run_number: 1,
            lane: Some(1),
            status: RunStatus::Completed,
            original_time_ms: Some(47_240),
            correction_ms: 0,
            started_at: None,
            ended_at: None,
            lineup: Vec::new(),
            lineup_confirmed_at: None,
            lineup_confirmed_by: None,
            time_source: TimeSource::Lichtschranke,
            hand_time_1_ms: None,
            hand_time_2_ms: None,
            penalties_accepted_at: None,
            created_at: t("2027-06-12T09:00:00Z"),
            updated_at: t("2027-06-12T09:00:00Z"),
        }
    }

    fn pending(run: &Run, kind: PenaltyKind) -> Penalty {
        Penalty {
            id: Uuid::new_v4(),
            event_id: run.event_id,
            discipline_id: run.discipline_id,
            run_id: run.id,
            team_id: run.team_id,
            runner_id: None,
            kind,
            code: None,
            station: None,
            is_disqualification: kind == PenaltyKind::Fv,
            decider_slot: 0,
            delta_ms: crate::models::penalty::catalogue_delta_ms(kind),
            reason_code: None,
            status: PenaltyStatus::Pending,
            created_at: t("2027-06-12T10:00:00Z"),
            reviewed_at: None,
            reviewed_by: None,
        }
    }

    #[test]
    fn router_builds_with_undo_route() {
        let _ = router();
    }

    #[test]
    fn decide_only_pending() {
        let r = run();
        let mut p = pending(&r, PenaltyKind::Sz);
        decide(
            &mut p,
            PenaltyStatus::Approved,
            t("2027-06-12T10:01:00Z"),
            "org",
        )
        .unwrap();
        assert_eq!(p.status, PenaltyStatus::Approved);
        assert_eq!(p.reviewed_at, Some(t("2027-06-12T10:01:00Z")));
        assert_eq!(p.reviewed_by.as_deref(), Some("org"));
        let err = decide(&mut p, PenaltyStatus::Rejected, Utc::now(), "org").unwrap_err();
        assert_eq!(err.code(), "conflict");
        assert_eq!(p.status, PenaltyStatus::Approved, "unchanged on 409");
    }

    #[test]
    fn undo_approved_back_to_pending_and_correction_recomputed() {
        let r = run();
        let mut p = pending(&r, PenaltyKind::Sz);
        decide(
            &mut p,
            PenaltyStatus::Approved,
            t("2027-06-12T10:01:00Z"),
            "org",
        )
        .unwrap();
        assert_eq!(legacy_correction_ms(r.id, [&p]), 15_000);

        undo_review(&mut p, &r).unwrap();
        assert_eq!(p.status, PenaltyStatus::Pending);
        assert_eq!(p.reviewed_at, None);
        assert_eq!(p.reviewed_by, None);
        assert_eq!(
            legacy_correction_ms(r.id, [&p]),
            0,
            "pending is not in correction_ms"
        );
    }

    #[test]
    fn undo_rejected_back_to_pending() {
        let r = run();
        let mut p = pending(&r, PenaltyKind::Fv);
        decide(
            &mut p,
            PenaltyStatus::Rejected,
            t("2027-06-12T10:01:00Z"),
            "org",
        )
        .unwrap();
        undo_review(&mut p, &r).unwrap();
        assert_eq!(p.status, PenaltyStatus::Pending);
        assert_eq!(p.reviewed_at, None);
        assert_eq!(p.reviewed_by, None);
        // ... and can be decided again.
        decide(
            &mut p,
            PenaltyStatus::Approved,
            t("2027-06-12T10:02:00Z"),
            "org",
        )
        .unwrap();
        assert_eq!(p.status, PenaltyStatus::Approved);
    }

    #[test]
    fn undo_pending_is_conflict() {
        let r = run();
        let mut p = pending(&r, PenaltyKind::Sz);
        let before = p.clone();
        let err = undo_review(&mut p, &r).unwrap_err();
        assert_eq!(err.code(), "conflict");
        assert!(format!("{err:?}").contains("noch offen"));
        assert_eq!(p, before);
    }

    #[test]
    fn undo_after_take_over_is_conflict() {
        let mut r = run();
        let mut decided = pending(&r, PenaltyKind::Sz);
        decide(
            &mut decided,
            PenaltyStatus::Rejected,
            t("2027-06-12T10:01:00Z"),
            "org",
        )
        .unwrap();
        let open = pending(&r, PenaltyKind::Sz);

        let changed = accept_run(
            &mut r,
            &[decided.clone(), open.clone()],
            t("2027-06-12T10:05:00Z"),
            "org",
        );
        assert_eq!(r.penalties_accepted_at, Some(t("2027-06-12T10:05:00Z")));
        assert_eq!(changed.len(), 1, "only the pending one");
        let mut bulk_approved = changed[0].clone();
        assert_eq!(bulk_approved.id, open.id);
        assert_eq!(bulk_approved.status, PenaltyStatus::Approved);
        assert_eq!(
            bulk_approved.reviewed_at, r.penalties_accepted_at,
            "same instant"
        );

        // Decided before Übernehmen → frozen.
        let before = decided.clone();
        let err = undo_review(&mut decided, &r).unwrap_err();
        assert_eq!(err.code(), "conflict");
        assert!(format!("{err:?}").contains("übernommen"));
        assert_eq!(decided, before);
        // Approved by Übernehmen itself → frozen (strict `>`).
        assert_eq!(
            undo_review(&mut bulk_approved, &r).unwrap_err().code(),
            "conflict"
        );
    }

    #[test]
    fn undo_works_for_decision_made_after_take_over() {
        let mut r = run();
        accept_run(&mut r, &[], t("2027-06-12T10:05:00Z"), "org");
        // A late judge report, decided after Übernehmen.
        let mut late = pending(&r, PenaltyKind::Sz);
        decide(
            &mut late,
            PenaltyStatus::Approved,
            t("2027-06-12T10:06:00Z"),
            "org",
        )
        .unwrap();
        undo_review(&mut late, &r).unwrap();
        assert_eq!(late.status, PenaltyStatus::Pending);

        // Pressing Übernehmen again freezes it.
        decide(
            &mut late,
            PenaltyStatus::Approved,
            t("2027-06-12T10:07:00Z"),
            "org",
        )
        .unwrap();
        accept_run(
            &mut r,
            std::slice::from_ref(&late),
            t("2027-06-12T10:08:00Z"),
            "org",
        );
        assert_eq!(
            r.penalties_accepted_at,
            Some(t("2027-06-12T10:08:00Z")),
            "updated on every press"
        );
        assert_eq!(undo_review(&mut late, &r).unwrap_err().code(), "conflict");
    }

    #[test]
    fn accept_run_only_touches_pending_of_that_run() {
        let mut r = run();
        let other = run();
        let mine = pending(&r, PenaltyKind::Fv);
        let foreign = pending(&other, PenaltyKind::Sz);
        let changed = accept_run(
            &mut r,
            &[mine.clone(), foreign],
            t("2027-06-12T10:05:00Z"),
            "org",
        );
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].id, mine.id);
        assert_eq!(changed[0].reviewed_by.as_deref(), Some("org"));
    }

    #[test]
    fn bulk_approve_response_carries_accepted_at() {
        let v = serde_json::to_value(BulkApproveResponse {
            approved: 2,
            rejected: 0,
            dq: 1,
            penalties_accepted_at: t("2027-06-12T10:05:00Z"),
        })
        .unwrap();
        assert_eq!(v["approved"], 2);
        assert_eq!(v["dq"], 1);
        assert_eq!(v["penalties_accepted_at"], "2027-06-12T10:05:00Z");
    }
}
