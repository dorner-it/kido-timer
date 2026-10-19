// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::auth::session::random_token;
use crate::db::{disciplines, events, runners, runs, team_entries, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Discipline, Event, Run, Runner, Team, TeamEntryView};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", get(detail).put(update).delete(remove))
        .route("/{id}/activate", post(activate))
        .route("/{id}/current", post(set_current))
        .route("/{id}/decider-tokens/rotate", post(rotate_decider_tokens))
}

// ===== helpers =====

pub async fn load_owned(state: &AppState, auth: &AuthenticatedUser, id: Uuid) -> AppResult<Event> {
    let ev = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("event {id}")))?;
    if ev.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }
    Ok(ev)
}

pub async fn require_owned(
    state: &AppState,
    auth: &AuthenticatedUser,
    id: Uuid,
) -> AppResult<Event> {
    load_owned(state, auth, id).await
}

fn fresh_decider_tokens() -> [String; 4] {
    [
        random_token(24),
        random_token(24),
        random_token(24),
        random_token(24),
    ]
}

/// 32 raw bytes → URL-safe base64-no-pad (~43 chars). Used as the
/// single shared strafrichter token in `/decider/:owner/:token/...`.
fn fresh_station_token() -> String {
    random_token(32)
}

// ===== list / create =====

pub async fn list(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
) -> AppResult<Json<Vec<Event>>> {
    let evs = events::list_for_owner(&state.db, &auth.user.sub).await?;
    Ok(Json(evs))
}

#[derive(Deserialize)]
pub struct CreateEventInput {
    pub name: String,
    pub date: NaiveDate,
    pub location: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(input): Json<CreateEventInput>,
) -> AppResult<(StatusCode, Json<Event>)> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("name must not be empty".into()));
    }
    let now = Utc::now();
    let ev = Event {
        id: Uuid::new_v4(),
        owner_sub: auth.user.sub,
        name: input.name.trim().to_string(),
        date: input.date,
        location: input
            .location
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()),
        is_active: false,
        current_discipline_id: None,
        current_run_id: None,
        current_run_ids: Vec::new(),
        decider_tokens: fresh_decider_tokens(),
        station_token: fresh_station_token(),
        created_at: now,
        updated_at: now,
    };
    events::put(&state.db, &ev).await?;
    Ok((StatusCode::CREATED, Json(ev)))
}

// ===== detail =====

#[derive(Serialize)]
pub struct EventDetail {
    pub event: Event,
    pub disciplines: Vec<Discipline>,
    pub teams: Vec<Team>,
    pub runners: Vec<Runner>,
    /// Entries incl. the computed `wg_warnings` (event year).
    pub team_entries: Vec<TeamEntryView>,
    pub runs: Vec<Run>,
}

pub async fn detail(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<EventDetail>> {
    let ev = load_owned(&state, &auth, id).await?;
    let discipline_list = disciplines::list_for_event(&state.db, id).await?;
    let team_list = teams::list_for_event(&state.db, id).await?;
    let runner_list = runners::list_for_event(&state.db, id).await?;
    let entry_list = team_entries::list_for_event(&state.db, id).await?;
    let run_list = runs::list_for_event(&state.db, id).await?;

    Ok(Json(EventDetail {
        disciplines: discipline_list,
        teams: team_list,
        team_entries: entry_views(entry_list, &runner_list, ev.date.year()),
        runners: runner_list,
        runs: run_list,
        event: ev,
    }))
}

/// Attach the computed `wg_warnings` to every entry.
pub fn entry_views(
    entries: Vec<crate::models::TeamEntry>,
    runners: &[Runner],
    competition_year: i32,
) -> Vec<TeamEntryView> {
    entries
        .into_iter()
        .map(|e| TeamEntryView::new(e, runners, competition_year))
        .collect()
}

// ===== update / delete =====

#[derive(Deserialize)]
pub struct UpdateEventInput {
    pub name: Option<String>,
    pub date: Option<NaiveDate>,
    pub location: Option<Option<String>>,
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateEventInput>,
) -> AppResult<Json<Event>> {
    let mut ev = load_owned(&state, &auth, id).await?;
    if let Some(name) = input.name {
        if name.trim().is_empty() {
            return Err(AppError::Validation("name must not be empty".into()));
        }
        ev.name = name.trim().to_string();
    }
    if let Some(date) = input.date {
        ev.date = date;
    }
    if let Some(location) = input.location {
        ev.location = location
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());
    }
    // Backfill `station_token` for legacy events that predate the
    // stations feature. Doing it on the first write after deployment
    // avoids a one-shot migration job and keeps QR codes minting on
    // the first edit.
    if ev.station_token.is_empty() {
        ev.station_token = fresh_station_token();
    }
    ev.updated_at = Utc::now();
    events::put(&state.db, &ev).await?;
    Ok(Json(ev))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    load_owned(&state, &auth, id).await?;
    events::cascade_delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===== activate / current =====

pub async fn activate(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Event>> {
    load_owned(&state, &auth, id).await?;
    events::activate_exclusive(&state.db, &auth.user.sub, id).await?;
    let updated = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("event vanished mid-activate".into()))?;
    Ok(Json(updated))
}

/// Body of `POST /api/events/{id}/current`. Accepts the legacy
/// `{ discipline_id, run_id }` **or** `{ discipline_id, run_ids: [..] }`.
#[derive(Debug, Default, Deserialize)]
pub struct SetCurrentInput {
    pub discipline_id: Option<Uuid>,
    /// Legacy single run.
    #[serde(default)]
    pub run_id: Option<Uuid>,
    /// One run per active lane (max `lane_count`).
    #[serde(default)]
    pub run_ids: Option<Vec<Uuid>>,
}

/// Turn a [`SetCurrentInput`] into the list of current run ids.
///
/// - `run_ids` wins when present (also when `run_id` is sent too, so
///   new clients may send both for old servers); otherwise
///   `run_id` → `[run_id]`; neither → `[]`.
/// - Runs require a discipline.
/// - No duplicates, at most `lane_count` runs.
/// - Every run must belong to the discipline (`discipline_run_ids`).
pub fn resolve_current_run_ids(
    input: &SetCurrentInput,
    lane_count: Option<u8>,
    discipline_run_ids: &[Uuid],
) -> AppResult<Vec<Uuid>> {
    let ids: Vec<Uuid> = match (&input.run_ids, input.run_id) {
        (Some(ids), _) => ids.clone(),
        (None, Some(id)) => vec![id],
        (None, None) => Vec::new(),
    };
    if ids.is_empty() {
        return Ok(ids);
    }
    let Some(lane_count) = lane_count.filter(|_| input.discipline_id.is_some()) else {
        return Err(AppError::Validation(
            "cannot set current runs without a current_discipline_id".into(),
        ));
    };
    if ids.len() > usize::from(lane_count) {
        return Err(AppError::Validation(format!(
            "höchstens {lane_count} aktuelle Läufe erlaubt (lane_count), {} angegeben",
            ids.len()
        )));
    }
    for (i, id) in ids.iter().enumerate() {
        if ids[..i].contains(id) {
            return Err(AppError::Validation(format!(
                "run {id} ist doppelt angegeben"
            )));
        }
        if !discipline_run_ids.contains(id) {
            return Err(AppError::Validation(format!(
                "run {id} gehört nicht zu diesem Wettkampf"
            )));
        }
    }
    Ok(ids)
}

pub async fn set_current(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(input): Json<SetCurrentInput>,
) -> AppResult<Json<Event>> {
    let ev = load_owned(&state, &auth, id).await?;
    let wants_runs =
        input.run_id.is_some() || input.run_ids.as_ref().is_some_and(|v| !v.is_empty());
    if input.discipline_id.is_none() && wants_runs {
        return Err(AppError::Validation(
            "cannot set current runs without a current_discipline_id".into(),
        ));
    }
    let mut lane_count = None;
    let mut discipline_run_ids = Vec::new();
    if let Some(did) = input.discipline_id {
        let disc = disciplines::find(&state.db, id, did)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("discipline {did}")))?;
        if disc.event_id != id {
            return Err(AppError::Validation(
                "discipline does not belong to this event".into(),
            ));
        }
        lane_count = Some(disc.lane_count);
        if wants_runs {
            discipline_run_ids = runs::list_for_discipline(&state.db, id, did)
                .await?
                .into_iter()
                .map(|r| r.id)
                .collect();
        }
    }
    let run_ids = resolve_current_run_ids(&input, lane_count, &discipline_run_ids)?;

    // Putting a discipline on the lane implies the event itself becomes
    // the active one (otherwise the decider pages would still see "no
    // active event"). Activating exclusively also deactivates other
    // events the user owns.
    if input.discipline_id.is_some() && !ev.is_active {
        events::activate_exclusive(&state.db, &auth.user.sub, id).await?;
    }

    events::set_current(&state.db, id, input.discipline_id, &run_ids).await?;
    let updated = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("event vanished mid-update".into()))?;
    Ok(Json(updated))
}

pub async fn rotate_decider_tokens(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Event>> {
    load_owned(&state, &auth, id).await?;
    events::rotate_decider_tokens(&state.db, id, fresh_decider_tokens()).await?;
    let updated = events::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("event vanished mid-rotate".into()))?;
    Ok(Json(updated))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(
        discipline_id: Option<Uuid>,
        run_id: Option<Uuid>,
        run_ids: Option<Vec<Uuid>>,
    ) -> SetCurrentInput {
        SetCurrentInput {
            discipline_id,
            run_id,
            run_ids,
        }
    }

    #[test]
    fn body_accepts_legacy_and_new_shapes() {
        let (d, r) = (Uuid::new_v4(), Uuid::new_v4());
        let legacy: SetCurrentInput =
            serde_json::from_value(serde_json::json!({ "discipline_id": d, "run_id": r })).unwrap();
        assert_eq!(legacy.run_id, Some(r));
        assert_eq!(legacy.run_ids, None);
        let new: SetCurrentInput =
            serde_json::from_value(serde_json::json!({ "discipline_id": d, "run_ids": [r] }))
                .unwrap();
        assert_eq!(new.run_ids, Some(vec![r]));
        let clear: SetCurrentInput =
            serde_json::from_value(serde_json::json!({ "discipline_id": null })).unwrap();
        assert_eq!(clear.discipline_id, None);
    }

    #[test]
    fn legacy_run_id_becomes_single_element_list() {
        let (d, r) = (Uuid::new_v4(), Uuid::new_v4());
        let ids = resolve_current_run_ids(&input(Some(d), Some(r), None), Some(2), &[r]).unwrap();
        assert_eq!(ids, vec![r]);
    }

    #[test]
    fn run_ids_up_to_lane_count() {
        let (d, a, b) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let ids =
            resolve_current_run_ids(&input(Some(d), None, Some(vec![a, b])), Some(2), &[a, b])
                .unwrap();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn run_ids_win_over_run_id() {
        let (d, a, b) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let ids =
            resolve_current_run_ids(&input(Some(d), Some(a), Some(vec![b])), Some(2), &[a, b])
                .unwrap();
        assert_eq!(ids, vec![b]);
    }

    #[test]
    fn more_runs_than_lanes_rejected() {
        let (d, a, b) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let err =
            resolve_current_run_ids(&input(Some(d), None, Some(vec![a, b])), Some(1), &[a, b])
                .unwrap_err();
        assert_eq!(err.code(), "validation");
    }

    #[test]
    fn duplicate_run_ids_rejected() {
        let (d, a) = (Uuid::new_v4(), Uuid::new_v4());
        let err = resolve_current_run_ids(&input(Some(d), None, Some(vec![a, a])), Some(2), &[a])
            .unwrap_err();
        assert_eq!(err.code(), "validation");
    }

    #[test]
    fn foreign_run_rejected() {
        let (d, a, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let err = resolve_current_run_ids(&input(Some(d), None, Some(vec![other])), Some(2), &[a])
            .unwrap_err();
        assert_eq!(err.code(), "validation");
    }

    #[test]
    fn runs_without_discipline_rejected() {
        let a = Uuid::new_v4();
        assert!(resolve_current_run_ids(&input(None, Some(a), None), None, &[a]).is_err());
        assert!(resolve_current_run_ids(&input(None, None, Some(vec![a])), None, &[a]).is_err());
    }

    #[test]
    fn empty_selection_clears() {
        let d = Uuid::new_v4();
        assert!(
            resolve_current_run_ids(&input(Some(d), None, None), Some(2), &[])
                .unwrap()
                .is_empty()
        );
        assert!(
            resolve_current_run_ids(&input(Some(d), None, Some(vec![])), Some(2), &[])
                .unwrap()
                .is_empty()
        );
        assert!(
            resolve_current_run_ids(&input(None, None, None), None, &[])
                .unwrap()
                .is_empty()
        );
    }
}
