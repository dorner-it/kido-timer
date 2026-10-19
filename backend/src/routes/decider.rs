// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete as method_delete, get, post, put};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kido_core::catalogue::{self, CatalogueEntry};

use crate::db::{disciplines, events, penalties, runners, runs, teams};
use crate::error::{AppError, AppResult};
use crate::models::penalty::catalogue_delta_ms;
use crate::models::{
    Discipline, Event, JudgeMode, LineupConfirmedBy, Penalty, PenaltyKind, PenaltyStatus, Run,
    Runner, StationKind, SyncMode, Team, TimerMode,
};
use crate::routes::runs::{LineupInput, apply_lineup, validate_lineup};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        // Legacy slot-token surface (kept so existing QR codes keep
        // working until the next event is created):
        .route("/{owner_sub}/{slot}/{token}/state", get(state_endpoint))
        .route(
            "/{owner_sub}/{slot}/{token}/penalties",
            post(submit_penalty),
        )
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
        .route(
            "/{owner_sub}/{token}/runs/{run_id}/lineup",
            put(set_station_lineup),
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
    /// Kept for old clients = `current_runs[0]`.
    pub current_run: Option<DeciderRunView>,
    /// One entry per current run, ordered by lane.
    pub current_runs: Vec<DeciderRunView>,
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
    pub mode: TimerMode,
    pub lane_count: u8,
    pub judge_mode: JudgeMode,
}

impl PublicDiscipline {
    fn from_discipline(d: &Discipline) -> Self {
        Self {
            id: d.id,
            name: d.name.clone(),
            mode: d.mode,
            lane_count: d.lane_count,
            judge_mode: d.judge_mode,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct DeciderRunView {
    pub run: Run,
    pub team: Team,
    /// Legacy: the run's single runner (individual modes). Usually `null`.
    pub runner: Option<Runner>,
    /// The team's full roster (incl. substitutes), by position — the
    /// Startlinie judge ticks the lineup from this list.
    pub runners: Vec<Runner>,
}

/// Keep only the runs listed in `current_ids` and order them by lane
/// (runs without a lane last, ties in `current_ids` order).
pub fn order_current_runs(runs: Vec<Run>, current_ids: &[Uuid]) -> Vec<Run> {
    let mut picked: Vec<(usize, Run)> = runs
        .into_iter()
        .filter_map(|r| {
            current_ids
                .iter()
                .position(|id| *id == r.id)
                .map(|i| (i, r))
        })
        .collect();
    picked.sort_by_key(|(i, r)| (r.lane.unwrap_or(u8::MAX), *i));
    picked.into_iter().map(|(_, r)| r).collect()
}

/// Resolve the run a judge submission targets.
///
/// - No current run → `409 conflict`.
/// - `run_id` given → must be one of `current_ids` (`409 conflict`
///   otherwise — the organizer moved on).
/// - `run_id` omitted → allowed only when exactly one run is current
///   (backwards compatible); with several current runs → `400
///   validation`.
pub fn resolve_target_run(requested: Option<Uuid>, current_ids: &[Uuid]) -> AppResult<Uuid> {
    if current_ids.is_empty() {
        return Err(AppError::Conflict("no run is currently active".into()));
    }
    match requested {
        Some(id) if current_ids.contains(&id) => Ok(id),
        Some(id) => Err(AppError::Conflict(format!(
            "run {id} ist nicht (mehr) aktuell"
        ))),
        None if current_ids.len() == 1 => Ok(current_ids[0]),
        None => Err(AppError::Validation(
            "run_id ist erforderlich, wenn mehrere Läufe aktuell sind".into(),
        )),
    }
}

/// Catalogue checks for a station submission: station allowed for the
/// discipline mode, code exists and belongs to that station.
pub fn validate_station_submission(
    mode: TimerMode,
    station: StationKind,
    code: &str,
) -> AppResult<&'static CatalogueEntry> {
    if !station.allowed_for(mode) {
        return Err(AppError::Validation(format!(
            "station {} is not valid for mode {mode:?}",
            station.as_str()
        )));
    }
    let entry = catalogue::lookup(code)
        .ok_or_else(|| AppError::Validation(format!("unbekannter Fehlercode {code}")))?;
    if entry.station != station {
        return Err(AppError::Validation(format!(
            "Fehlercode {code} gehört nicht zur Station {}",
            station.as_str()
        )));
    }
    Ok(entry)
}

/// Build the penalty row for a catalogue submission. `existing` is the
/// pending row with the same `client_id` (idempotent re-submit).
pub fn build_station_penalty(
    id: Uuid,
    run: &Run,
    station: StationKind,
    entry: &CatalogueEntry,
    existing: Option<&Penalty>,
    now: chrono::DateTime<Utc>,
) -> Penalty {
    Penalty {
        id,
        event_id: run.event_id,
        discipline_id: run.discipline_id,
        run_id: run.id,
        team_id: run.team_id,
        runner_id: run.runner_id,
        kind: entry.kind,
        code: Some(entry.code.to_string()),
        station: Some(station),
        is_disqualification: entry.kind == PenaltyKind::Fv,
        // No slot identity in the station flow — 0 is the sentinel.
        decider_slot: existing.map(|p| p.decider_slot).unwrap_or(0),
        delta_ms: catalogue_delta_ms(entry.kind),
        reason_code: None,
        status: PenaltyStatus::Pending,
        created_at: existing.map(|p| p.created_at).unwrap_or(now),
        reviewed_at: None,
        reviewed_by: None,
    }
}

/// Load the current runs of `disc` with team + roster, ordered by lane.
async fn load_current_runs(
    state: &AppState,
    event: &Event,
    disc: &Discipline,
) -> AppResult<Vec<DeciderRunView>> {
    if event.current_run_ids.is_empty() {
        return Ok(Vec::new());
    }
    let runs_for_disc = runs::list_for_discipline(&state.db, event.id, disc.id).await?;
    let current = order_current_runs(runs_for_disc, &event.current_run_ids);
    if current.is_empty() {
        return Ok(Vec::new());
    }
    let team_list = teams::list_for_event(&state.db, event.id).await?;
    let runner_list = runners::list_for_event(&state.db, event.id).await?;

    let mut out = Vec::with_capacity(current.len());
    for run in current {
        let team = team_list
            .iter()
            .find(|t| t.id == run.team_id)
            .cloned()
            .ok_or_else(|| AppError::Internal("run references unknown team".into()))?;
        let mut roster: Vec<Runner> = runner_list
            .iter()
            .filter(|r| r.team_id == run.team_id)
            .cloned()
            .collect();
        roster.sort_by_key(|r| r.position);
        let runner = run
            .runner_id
            .and_then(|rid| roster.iter().find(|r| r.id == rid).cloned());
        out.push(DeciderRunView {
            run,
            team,
            runner,
            runners: roster,
        });
    }
    Ok(out)
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
            current_runs: Vec::new(),
            disabled: true,
        }));
    };

    let public_disc = PublicDiscipline::from_discipline(&disc);

    if disc.sync_mode == SyncMode::Offline {
        return Ok(Json(DeciderState {
            event: public_event,
            discipline: Some(public_disc),
            slot,
            current_run: None,
            current_runs: Vec::new(),
            disabled: true,
        }));
    }

    let current_runs = load_current_runs(&state, &event, &disc).await?;
    let current_run = current_runs.first().cloned();

    Ok(Json(DeciderState {
        event: public_event,
        discipline: Some(public_disc),
        slot,
        current_run,
        current_runs,
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

    // The slot QR code knows no run: works while exactly one run is
    // current.
    if event.current_run_ids.len() > 1 {
        return Err(AppError::Conflict(
            "mehrere Läufe sind aktuell; der Slot-QR-Code kann keinen Lauf wählen — bitte den Stations-QR-Code verwenden".into(),
        ));
    }
    let run_id = resolve_target_run(None, &event.current_run_ids)?;
    let run = runs::find(&state.db, event.id, disc.id, run_id)
        .await?
        .ok_or_else(|| AppError::Internal("current_run_ids references unknown run".into()))?;

    let penalty_id = input.client_id.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();

    let existing = penalties::get(&state.db, event.id, disc.id, penalty_id).await?;
    if let Some(existing) = existing.as_ref() {
        if existing.decider_slot != slot {
            return Err(AppError::Conflict(
                "client_id belongs to a different slot".into(),
            ));
        }
        if existing.status != PenaltyStatus::Pending {
            return Ok((StatusCode::OK, Json(existing.clone())));
        }
    }
    let penalty = build_slot_penalty(penalty_id, &run, slot, &input, existing.as_ref(), now);
    penalties::put(&state.db, &penalty).await?;
    let code = if existing.is_some() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((code, Json(penalty)))
}

/// Penalty row for the legacy slot route: always `kind = sz`,
/// `code = null`, `delta_ms` exactly as sent, no station.
pub fn build_slot_penalty(
    id: Uuid,
    run: &Run,
    slot: u8,
    input: &SubmitPenaltyInput,
    existing: Option<&Penalty>,
    now: chrono::DateTime<Utc>,
) -> Penalty {
    Penalty {
        id,
        event_id: run.event_id,
        discipline_id: run.discipline_id,
        run_id: run.id,
        team_id: run.team_id,
        runner_id: run.runner_id,
        kind: PenaltyKind::Sz,
        code: None,
        station: existing.and_then(|p| p.station),
        is_disqualification: false,
        decider_slot: slot,
        delta_ms: input.delta_ms,
        reason_code: input
            .reason_code
            .as_ref()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty()),
        status: PenaltyStatus::Pending,
        created_at: existing.map(|p| p.created_at).unwrap_or(now),
        reviewed_at: None,
        reviewed_by: None,
    }
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
        return Err(AppError::Conflict(
            "only pending penalties can be withdrawn".into(),
        ));
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
    /// Kept for old clients = `current_runs[0]`.
    pub current_run: Option<DeciderRunView>,
    /// NEW: one entry per current run, ordered by lane.
    pub current_runs: Vec<DeciderRunView>,
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
            current_runs: Vec::new(),
            disabled: true,
            allowed_stations: Vec::new(),
        }));
    };

    let public_disc = PublicDiscipline::from_discipline(&disc);
    let allowed = StationKind::for_mode(disc.mode).to_vec();

    if disc.sync_mode == SyncMode::Offline {
        return Ok(Json(StationDeciderState {
            event: public_event,
            discipline: Some(public_disc),
            current_run: None,
            current_runs: Vec::new(),
            disabled: true,
            allowed_stations: allowed,
        }));
    }

    let current_runs = load_current_runs(&state, &event, &disc).await?;
    let current_run = current_runs.first().cloned();

    Ok(Json(StationDeciderState {
        event: public_event,
        discipline: Some(public_disc),
        current_run,
        current_runs,
        disabled: false,
        allowed_stations: allowed,
    }))
}

/// Body of the station submit. `kind` and `delta_ms` come from the
/// catalogue entry of `code`.
#[derive(Debug, Deserialize)]
pub struct SubmitStationPenaltyInput {
    pub station: StationKind,
    /// Target run; optional only while exactly one run is current.
    #[serde(default)]
    pub run_id: Option<Uuid>,
    /// Catalogue code (`"<station>.<slug>"`). Required — `Option` only
    /// so a missing value yields `400 validation`.
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub client_id: Option<Uuid>,
}

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
    let code = input
        .code
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| AppError::Validation("code (Fehlercode) muss angegeben werden".into()))?;
    let entry = validate_station_submission(disc.mode, input.station, code)?;
    let run_id = resolve_target_run(input.run_id, &event.current_run_ids)?;
    let run = runs::find(&state.db, event.id, disc.id, run_id)
        .await?
        .ok_or_else(|| AppError::Internal("current_run_ids references unknown run".into()))?;

    let penalty_id = input.client_id.unwrap_or_else(Uuid::new_v4);
    let existing = penalties::get(&state.db, event.id, disc.id, penalty_id).await?;
    // Idempotency: a re-submit with the same client_id upserts the row
    // while still pending, or returns the already-reviewed copy.
    if let Some(existing) = existing.as_ref() {
        if existing.status != PenaltyStatus::Pending {
            return Ok((StatusCode::OK, Json(existing.clone())));
        }
    }
    let penalty = build_station_penalty(
        penalty_id,
        &run,
        input.station,
        entry,
        existing.as_ref(),
        Utc::now(),
    );
    penalties::put(&state.db, &penalty).await?;
    let status = if existing.is_some() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(penalty)))
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

// ===== judge lineup (Startlinie) =====

/// Body of the judge lineup endpoint. `station` is optional: when the
/// client sends it, it must be a station that records lineups
/// (`startlinie`).
#[derive(Debug, Deserialize)]
pub struct StationLineupInput {
    #[serde(flatten)]
    pub lineup: LineupInput,
    #[serde(default)]
    pub station: Option<StationKind>,
}

/// Only the Startlinie judge may confirm a lineup.
pub fn check_lineup_station(station: Option<StationKind>) -> AppResult<()> {
    match station {
        Some(s) if !s.records_lineup() => Err(AppError::Forbidden),
        _ => Ok(()),
    }
}

/// `PUT /api/decider/{owner_sub}/{token}/runs/{run_id}/lineup` — the
/// Startlinie judge confirms who actually runs. The run must be one of
/// the event's `current_run_ids`.
pub async fn set_station_lineup(
    State(state): State<AppState>,
    Path((owner_sub, token, run_id)): Path<(String, String, Uuid)>,
    Json(input): Json<StationLineupInput>,
) -> AppResult<Json<Run>> {
    let (event, current_disc) = authorize_station(&state, &owner_sub, &token).await?;
    let disc = current_disc.ok_or_else(|| {
        AppError::Conflict("no discipline is currently selected on the lane".into())
    })?;
    if disc.sync_mode == SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf läuft offline; Strafrichter sind deaktiviert".into(),
        ));
    }
    check_lineup_station(input.station)?;
    let run_id = resolve_target_run(Some(run_id), &event.current_run_ids)?;
    let mut run = runs::find(&state.db, event.id, disc.id, run_id)
        .await?
        .ok_or_else(|| AppError::Internal("current_run_ids references unknown run".into()))?;
    let roster = runners::list_for_team(&state.db, event.id, run.team_id).await?;
    validate_lineup(&input.lineup.runner_ids, &roster, run.team_id)?;
    apply_lineup(
        &mut run,
        input.lineup.runner_ids,
        LineupConfirmedBy::Startlinie,
        Utc::now(),
    );
    runs::put(&state.db, &run).await?;
    Ok(Json(run))
}
