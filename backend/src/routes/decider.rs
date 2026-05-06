// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete as method_delete, get, post};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{competitions, penalties, runners, runs, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Competition, Penalty, PenaltyStatus, Run, Runner, SyncMode, Team};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{owner_sub}/{slot}/{token}/state", get(state_endpoint))
        .route("/{owner_sub}/{slot}/{token}/penalties", post(submit_penalty))
        .route(
            "/{owner_sub}/{slot}/{token}/penalties/{penalty_id}",
            method_delete(withdraw_penalty),
        )
}

/// Authorize a decider request and return the user's active competition.
async fn authorize(
    state: &AppState,
    owner_sub: &str,
    slot: u8,
    token: &str,
) -> AppResult<Competition> {
    if !(1..=4).contains(&slot) {
        return Err(AppError::Validation("slot must be 1..=4".into()));
    }

    let comps = competitions::list_for_owner(&state.db, owner_sub).await?;
    let active = comps
        .into_iter()
        .find(|c| c.is_active)
        .ok_or_else(|| AppError::NotFound("no active competition".into()))?;

    let expected = active
        .decider_tokens
        .get((slot - 1) as usize)
        .ok_or_else(|| AppError::Validation("slot out of range".into()))?;

    if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
        return Err(AppError::Unauthorized);
    }

    Ok(active)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ===== state endpoint =====

#[derive(Serialize)]
pub struct DeciderState {
    pub competition: PublicCompetition,
    pub slot: u8,
    pub current_run: Option<DeciderRunView>,
    /// True iff the competition is currently in `offline` sync mode. The
    /// pad UI renders a stub when this is set; penalty submissions are
    /// also rejected server-side in that state.
    pub disabled: bool,
}

#[derive(Serialize)]
pub struct PublicCompetition {
    pub id: Uuid,
    pub name: String,
    pub mode: crate::models::TimerMode,
}

#[derive(Serialize)]
pub struct DeciderRunView {
    pub run: Run,
    pub team: Team,
    pub runner: Option<Runner>,
}

pub async fn state_endpoint(
    State(state): State<AppState>,
    Path((owner_sub, slot, token)): Path<(String, u8, String)>,
) -> AppResult<Json<DeciderState>> {
    let active = authorize(&state, &owner_sub, slot, &token).await?;

    if active.sync_mode == SyncMode::Offline {
        return Ok(Json(DeciderState {
            competition: PublicCompetition {
                id: active.id,
                name: active.name,
                mode: active.mode,
            },
            slot,
            current_run: None,
            disabled: true,
        }));
    }

    let current_run = if let Some(run_id) = active.current_run_id {
        let runs_for_comp = runs::list_for_competition(&state.db, active.id).await?;
        let run = runs_for_comp.into_iter().find(|r| r.id == run_id);
        match run {
            Some(run) => {
                let team = teams::list_for_competition(&state.db, active.id)
                    .await?
                    .into_iter()
                    .find(|t| t.id == run.team_id)
                    .ok_or_else(|| AppError::Internal("run references unknown team".into()))?;
                let runner = match run.runner_id {
                    Some(rid) => runners::list_for_team(&state.db, active.id, run.team_id)
                        .await?
                        .into_iter()
                        .find(|r| r.id == rid),
                    None => None,
                };
                Some(DeciderRunView { run, team, runner })
            }
            None => None,
        }
    } else {
        None
    };

    Ok(Json(DeciderState {
        competition: PublicCompetition {
            id: active.id,
            name: active.name,
            mode: active.mode,
        },
        slot,
        current_run,
        disabled: false,
    }))
}

// ===== submit penalty =====

#[derive(Deserialize)]
pub struct SubmitPenaltyInput {
    pub delta_ms: i32,
    pub reason_code: Option<String>,
    /// Optional client-generated UUID. Used as the penalty's `id` so a
    /// retry from a flaky tablet upserts the same row instead of creating
    /// a duplicate. Absent → server generates one (legacy behaviour).
    pub client_id: Option<Uuid>,
}

pub async fn submit_penalty(
    State(state): State<AppState>,
    Path((owner_sub, slot, token)): Path<(String, u8, String)>,
    Json(input): Json<SubmitPenaltyInput>,
) -> AppResult<(StatusCode, Json<Penalty>)> {
    let active = authorize(&state, &owner_sub, slot, &token).await?;

    if active.sync_mode == SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf läuft offline; Strafrichter sind deaktiviert".into(),
        ));
    }

    let run_id = active
        .current_run_id
        .ok_or_else(|| AppError::Conflict("no run is currently active".into()))?;

    let runs_for_comp = runs::list_for_competition(&state.db, active.id).await?;
    let run = runs_for_comp
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| AppError::Internal("current_run_id references unknown run".into()))?;

    let penalty_id = input.client_id.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();

    // If a penalty with this client_id already exists, the request is a
    // retry. Two cases to handle:
    //  - Still pending: upsert with fresh inputs, preserve created_at.
    //  - Already reviewed (approved/rejected): return the existing row
    //    untouched. Otherwise a stale tablet retry could undo the
    //    organizer's review.
    if let Some(existing) = penalties::get(&state.db, active.id, penalty_id).await? {
        if existing.decider_slot != slot {
            return Err(AppError::Conflict(
                "client_id belongs to a different slot".into(),
            ));
        }
        if existing.status != PenaltyStatus::Pending {
            return Ok((StatusCode::OK, Json(existing)));
        }
        let penalty = Penalty {
            id: existing.id,
            competition_id: active.id,
            run_id: run.id,
            team_id: run.team_id,
            runner_id: run.runner_id,
            decider_slot: slot,
            delta_ms: input.delta_ms,
            reason_code: input
                .reason_code
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty()),
            status: PenaltyStatus::Pending,
            created_at: existing.created_at,
            reviewed_at: None,
            reviewed_by: None,
        };
        penalties::put(&state.db, &penalty).await?;
        return Ok((StatusCode::OK, Json(penalty)));
    }

    let penalty = Penalty {
        id: penalty_id,
        competition_id: active.id,
        run_id: run.id,
        team_id: run.team_id,
        runner_id: run.runner_id,
        decider_slot: slot,
        delta_ms: input.delta_ms,
        reason_code: input
            .reason_code
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty()),
        status: PenaltyStatus::Pending,
        created_at: now,
        reviewed_at: None,
        reviewed_by: None,
    };
    penalties::put(&state.db, &penalty).await?;

    Ok((StatusCode::CREATED, Json(penalty)))
}

// ===== withdraw penalty =====

pub async fn withdraw_penalty(
    State(state): State<AppState>,
    Path((owner_sub, slot, token, penalty_id)): Path<(String, u8, String, Uuid)>,
) -> AppResult<StatusCode> {
    let active = authorize(&state, &owner_sub, slot, &token).await?;

    if active.sync_mode == SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf läuft offline; Strafrichter sind deaktiviert".into(),
        ));
    }

    let penalty = penalties::get(&state.db, active.id, penalty_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("penalty {penalty_id}")))?;

    if penalty.decider_slot != slot {
        return Err(AppError::Forbidden);
    }
    if penalty.status != PenaltyStatus::Pending {
        return Err(AppError::Conflict("only pending penalties can be withdrawn".into()));
    }

    penalties::delete(&state.db, active.id, penalty_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
