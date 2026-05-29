// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::BTreeSet;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{penalties, runs};
use crate::error::{AppError, AppResult};
use crate::models::{Penalty, PenaltyStatus};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/penalties", get(list))
        .route("/{id}/penalties/{penalty_id}/approve", post(approve))
        .route("/{id}/penalties/{penalty_id}/reject", post(reject))
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

    if penalty.status != PenaltyStatus::Pending {
        return Err(AppError::Conflict(format!(
            "penalty already {}",
            penalty.status.as_str()
        )));
    }

    penalty.status = PenaltyStatus::Approved;
    penalty.reviewed_at = Some(Utc::now());
    penalty.reviewed_by = Some(auth.user.sub.clone());
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

    if penalty.status != PenaltyStatus::Pending {
        return Err(AppError::Conflict(format!(
            "penalty already {}",
            penalty.status.as_str()
        )));
    }

    penalty.status = PenaltyStatus::Rejected;
    penalty.reviewed_at = Some(Utc::now());
    penalty.reviewed_by = Some(auth.user.sub.clone());
    penalties::put(&state.db, &penalty).await?;

    Ok(Json(penalty))
}

async fn find_penalty(
    state: &AppState,
    event_id: Uuid,
    penalty_id: Uuid,
) -> AppResult<Penalty> {
    penalties::list_for_event(&state.db, event_id)
        .await?
        .into_iter()
        .find(|p| p.id == penalty_id)
        .ok_or_else(|| AppError::NotFound(format!("penalty {penalty_id}")))
}

// ===== bulk-approve =====
//
// "Lauf abschließen": approve every pending penalty for one run in a
// single click. We deliberately do NOT auto-reject anything — only the
// organiser decides what to throw away. The response keeps a `rejected`
// counter for the response shape's stability (always 0 today; reserved
// for a future "approve good / reject the rest" combined action).

#[derive(Serialize)]
pub struct BulkApproveResponse {
    pub approved: u32,
    pub rejected: u32,
    pub dq: u32,
}

pub async fn bulk_approve(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<BulkApproveResponse>> {
    require_owned(&state, &auth, event_id).await?;

    let all = penalties::list_for_event(&state.db, event_id).await?;
    let pending_for_run: Vec<Penalty> = all
        .into_iter()
        .filter(|p| p.run_id == run_id && p.status == PenaltyStatus::Pending)
        .collect();

    let mut approved: u32 = 0;
    let mut dq: u32 = 0;
    // Recompute correction once per (discipline, run) at the end
    // instead of after every single put — the cheap reads dominate.
    let mut touched: BTreeSet<(Uuid, Uuid)> = BTreeSet::new();
    let now = Utc::now();
    let reviewer = auth.user.sub.clone();

    for mut p in pending_for_run {
        p.status = PenaltyStatus::Approved;
        p.reviewed_at = Some(now);
        p.reviewed_by = Some(reviewer.clone());
        if p.is_disqualification {
            dq += 1;
        }
        touched.insert((p.discipline_id, p.run_id));
        penalties::put(&state.db, &p).await?;
        approved += 1;
    }

    for (discipline_id, run_id) in touched {
        recompute_run_correction(&state, event_id, discipline_id, run_id).await?;
    }

    Ok(Json(BulkApproveResponse {
        approved,
        rejected: 0,
        dq,
    }))
}

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
