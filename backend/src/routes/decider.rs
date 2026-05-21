// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete as method_delete, get, post};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{disciplines, events, penalties, runners, runs, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Discipline, Event, Penalty, PenaltyStatus, Run, Runner, SyncMode, Team};
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

/// Authorize a decider request and return the user's active event +
/// (if any) the discipline that's currently on the lane.
async fn authorize(
    state: &AppState,
    owner_sub: &str,
    slot: u8,
    token: &str,
) -> AppResult<(Event, Option<Discipline>)> {
    if !(1..=4).contains(&slot) {
        return Err(AppError::Validation("slot must be 1..=4".into()));
    }

    let evs = events::list_for_owner(&state.db, owner_sub).await?;
    let active = evs
        .into_iter()
        .find(|e| e.is_active)
        .ok_or_else(|| AppError::NotFound("no active event".into()))?;

    let expected = active
        .decider_tokens
        .get((slot - 1) as usize)
        .ok_or_else(|| AppError::Validation("slot out of range".into()))?;

    if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
        return Err(AppError::Unauthorized);
    }

    let current_disc = match active.current_discipline_id {
        Some(did) => disciplines::find(&state.db, active.id, did).await?,
        None => None,
    };

    Ok((active, current_disc))
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
    pub event: PublicEvent,
    /// The discipline currently on the lane. `None` when no discipline
    /// is selected yet ("Warte auf Wettkampf").
    pub discipline: Option<PublicDiscipline>,
    pub slot: u8,
    pub current_run: Option<DeciderRunView>,
    /// True iff the active discipline is in `offline` sync mode (or no
    /// discipline is selected). The pad UI renders a stub when this is
    /// set; penalty submissions are also rejected server-side then.
    pub disabled: bool,
}

#[derive(Serialize)]
pub struct PublicEvent {
    pub id: Uuid,
    pub name: String,
    pub date: chrono::NaiveDate,
    pub location: Option<String>,
}

#[derive(Serialize)]
pub struct PublicDiscipline {
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
    let (event, current_disc) = authorize(&state, &owner_sub, slot, &token).await?;

    let public_event = PublicEvent {
        id: event.id,
        name: event.name.clone(),
        date: event.date,
        location: event.location.clone(),
    };

    let Some(disc) = current_disc else {
        return Ok(Json(DeciderState {
            event: public_event,
            discipline: None,
            slot,
            current_run: None,
            disabled: true,
        }));
    };

    let public_disc = PublicDiscipline {
        id: disc.id,
        name: disc.name.clone(),
        mode: disc.mode,
    };

    if disc.sync_mode == SyncMode::Offline {
        return Ok(Json(DeciderState {
            event: public_event,
            discipline: Some(public_disc),
            slot,
            current_run: None,
            disabled: true,
        }));
    }

    let current_run = if let Some(run_id) = event.current_run_id {
        let runs_for_disc = runs::list_for_discipline(&state.db, event.id, disc.id).await?;
        let run = runs_for_disc.into_iter().find(|r| r.id == run_id);
        match run {
            Some(run) => {
                let team = teams::list_for_event(&state.db, event.id)
                    .await?
                    .into_iter()
                    .find(|t| t.id == run.team_id)
                    .ok_or_else(|| AppError::Internal("run references unknown team".into()))?;
                let runner = match run.runner_id {
                    Some(rid) => runners::list_for_team(&state.db, event.id, run.team_id)
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
        event: public_event,
        discipline: Some(public_disc),
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
    pub client_id: Option<Uuid>,
}

pub async fn submit_penalty(
    State(state): State<AppState>,
    Path((owner_sub, slot, token)): Path<(String, u8, String)>,
    Json(input): Json<SubmitPenaltyInput>,
) -> AppResult<(StatusCode, Json<Penalty>)> {
    let (event, current_disc) = authorize(&state, &owner_sub, slot, &token).await?;
    let disc = current_disc.ok_or_else(|| {
        AppError::Conflict("no discipline is currently selected on the lane".into())
    })?;

    if disc.sync_mode == SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf läuft offline; Strafrichter sind deaktiviert".into(),
        ));
    }

    let run_id = event
        .current_run_id
        .ok_or_else(|| AppError::Conflict("no run is currently active".into()))?;

    let runs_for_disc = runs::list_for_discipline(&state.db, event.id, disc.id).await?;
    let run = runs_for_disc
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| AppError::Internal("current_run_id references unknown run".into()))?;

    let penalty_id = input.client_id.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();

    if let Some(existing) = penalties::get(&state.db, event.id, disc.id, penalty_id).await? {
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
            event_id: event.id,
            discipline_id: disc.id,
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
        event_id: event.id,
        discipline_id: disc.id,
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
    let (event, current_disc) = authorize(&state, &owner_sub, slot, &token).await?;
    let disc = current_disc
        .ok_or_else(|| AppError::Conflict("no discipline is currently selected".into()))?;

    if disc.sync_mode == SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf läuft offline; Strafrichter sind deaktiviert".into(),
        ));
    }

    let penalty = penalties::get(&state.db, event.id, disc.id, penalty_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("penalty {penalty_id}")))?;

    if penalty.decider_slot != slot {
        return Err(AppError::Forbidden);
    }
    if penalty.status != PenaltyStatus::Pending {
        return Err(AppError::Conflict("only pending penalties can be withdrawn".into()));
    }

    penalties::delete(&state.db, event.id, disc.id, penalty_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
