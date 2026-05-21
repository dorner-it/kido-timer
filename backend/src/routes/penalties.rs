// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use chrono::Utc;
use serde::Deserialize;
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
