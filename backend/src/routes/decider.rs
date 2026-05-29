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
use crate::models::{
    Discipline, Event, Penalty, PenaltyStatus, Run, Runner, StationKind, SyncMode, Team,
};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        // Legacy slot-token surface (kept so existing QR codes keep
        // working until the next event is created):
        .route("/{owner_sub}/{slot}/{token}/state", get(state_endpoint))
        .route("/{owner_sub}/{slot}/{token}/penalties", post(submit_penalty))
        .route(
            "/{owner_sub}/{slot}/{token}/penalties/{penalty_id}",
            method_delete(withdraw_penalty),
        )
        // Stations flow: one shared token, station chosen in-app.
        .route("/{owner_sub}/{token}/state", get(station_state_endpoint))
        .route(
            "/{owner_sub}/{token}/penalties",
            post(submit_station_penalty),
        )
        .route(
            "/{owner_sub}/{token}/penalties/{penalty_id}",
            method_delete(withdraw_station_penalty),
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
            station: existing.station,
            is_disqualification: existing.is_disqualification,
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
        station: None,
        is_disqualification: false,
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

// ===== station-token surface ============================================
//
// New flow: one URL `/decider/<owner>/<station_token>` for *every*
// strafrichter. The role/identity used to be encoded in the URL slot;
// now the operator picks their `station` once after opening the page
// and the client stamps it onto every penalty.

/// Authorise an `owner / station_token` request. Same active-event +
/// constant-time-token logic as the slot flow, but keyed off the
/// event's `station_token` field.
async fn authorize_station(
    state: &AppState,
    owner_sub: &str,
    token: &str,
) -> AppResult<(Event, Option<Discipline>)> {
    let evs = events::list_for_owner(&state.db, owner_sub).await?;
    let active = evs
        .into_iter()
        .find(|e| e.is_active)
        .ok_or_else(|| AppError::NotFound("no active event".into()))?;

    if active.station_token.is_empty() {
        return Err(AppError::Conflict(
            "event has no station_token yet (save the event once to mint one)".into(),
        ));
    }
    if !constant_time_eq(token.as_bytes(), active.station_token.as_bytes()) {
        return Err(AppError::Unauthorized);
    }

    let current_disc = match active.current_discipline_id {
        Some(did) => disciplines::find(&state.db, active.id, did).await?,
        None => None,
    };
    Ok((active, current_disc))
}

#[derive(Serialize)]
pub struct StationDeciderState {
    pub event: PublicEvent,
    pub discipline: Option<PublicDiscipline>,
    pub current_run: Option<DeciderRunView>,
    pub disabled: bool,
    /// Stations the current discipline allows. Empty when no discipline
    /// is on the lane (the client then shows "waiting for organiser").
    pub allowed_stations: Vec<StationKind>,
}

pub async fn station_state_endpoint(
    State(state): State<AppState>,
    Path((owner_sub, token)): Path<(String, String)>,
) -> AppResult<Json<StationDeciderState>> {
    let (event, current_disc) = authorize_station(&state, &owner_sub, &token).await?;

    let public_event = PublicEvent {
        id: event.id,
        name: event.name.clone(),
        date: event.date,
        location: event.location.clone(),
    };

    let Some(disc) = current_disc else {
        return Ok(Json(StationDeciderState {
            event: public_event,
            discipline: None,
            current_run: None,
            disabled: true,
            allowed_stations: Vec::new(),
        }));
    };

    let public_disc = PublicDiscipline {
        id: disc.id,
        name: disc.name.clone(),
        mode: disc.mode,
    };
    let allowed = StationKind::for_mode(disc.mode);

    if disc.sync_mode == SyncMode::Offline {
        return Ok(Json(StationDeciderState {
            event: public_event,
            discipline: Some(public_disc),
            current_run: None,
            disabled: true,
            allowed_stations: allowed,
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

    Ok(Json(StationDeciderState {
        event: public_event,
        discipline: Some(public_disc),
        current_run,
        disabled: false,
        allowed_stations: allowed,
    }))
}

#[derive(Deserialize)]
pub struct SubmitStationPenaltyInput {
    pub station: StationKind,
    /// Time delta to add to the run's correction. Omit (`None`) when
    /// the submission is a disqualification — the server then stamps
    /// the sentinel `DSQ_DELTA_MS` value so existing time-based code
    /// paths keep working.
    pub delta_ms: Option<i32>,
    #[serde(default)]
    pub is_disqualification: bool,
    pub reason_code: Option<String>,
    pub client_id: Option<Uuid>,
}

/// 999 s. Matches `decider-presets.ts::COMMON_DSQ.delta_ms` so the
/// desktop export keeps treating these rows the same way it does now.
pub const DSQ_DELTA_MS: i32 = 999_000;

pub async fn submit_station_penalty(
    State(state): State<AppState>,
    Path((owner_sub, token)): Path<(String, String)>,
    Json(input): Json<SubmitStationPenaltyInput>,
) -> AppResult<(StatusCode, Json<Penalty>)> {
    let (event, current_disc) = authorize_station(&state, &owner_sub, &token).await?;
    let disc = current_disc.ok_or_else(|| {
        AppError::Conflict("no discipline is currently selected on the lane".into())
    })?;

    if disc.sync_mode == SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf läuft offline; Strafrichter sind deaktiviert".into(),
        ));
    }
    if !input.station.allowed_for(disc.mode) {
        return Err(AppError::Validation(format!(
            "station {} is not valid for mode {:?}",
            input.station.as_str(),
            disc.mode
        )));
    }
    if input.is_disqualification && input.delta_ms.is_some() {
        return Err(AppError::Validation(
            "delta_ms must be omitted when is_disqualification is true".into(),
        ));
    }
    if !input.is_disqualification && input.delta_ms.is_none() {
        return Err(AppError::Validation(
            "either delta_ms or is_disqualification must be provided".into(),
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

    let delta_ms = if input.is_disqualification {
        DSQ_DELTA_MS
    } else {
        input.delta_ms.unwrap_or(0)
    };

    let penalty_id = input.client_id.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();

    if let Some(existing) = penalties::get(&state.db, event.id, disc.id, penalty_id).await? {
        // Same idempotency contract as the slot flow: a second PUT
        // with the same client_id upserts the row when still pending,
        // or returns the already-reviewed copy unchanged.
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
            station: Some(input.station),
            is_disqualification: input.is_disqualification,
            decider_slot: existing.decider_slot,
            delta_ms,
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
        station: Some(input.station),
        is_disqualification: input.is_disqualification,
        // No slot identity in the new flow — leave 0 as a sentinel.
        decider_slot: 0,
        delta_ms,
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

#[derive(Deserialize)]
pub struct WithdrawStationQuery {
    /// Station the client *thinks* the penalty was filed under. Used
    /// as a soft validation against the LocalStorage state on the
    /// device — prevents a stale tab from accidentally pulling a
    /// neighbour's submission when both are logged in to the same
    /// shared `station_token`.
    pub station: Option<StationKind>,
}

pub async fn withdraw_station_penalty(
    State(state): State<AppState>,
    Path((owner_sub, token, penalty_id)): Path<(String, String, Uuid)>,
    axum::extract::Query(query): axum::extract::Query<WithdrawStationQuery>,
) -> AppResult<StatusCode> {
    let (event, current_disc) = authorize_station(&state, &owner_sub, &token).await?;
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

    if penalty.status != PenaltyStatus::Pending {
        return Err(AppError::Conflict(
            "only pending penalties can be withdrawn".into(),
        ));
    }
    if let (Some(requested), Some(actual)) = (query.station, penalty.station) {
        if requested != actual {
            return Err(AppError::Forbidden);
        }
    }

    penalties::delete(&state.db, event.id, disc.id, penalty_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
