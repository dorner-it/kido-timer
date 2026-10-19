// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Desktop-facing API mounted at `/api/export/...`.
//!
//! Authenticated via the `X-API-Key` header (per-user, argon2id-hashed
//! in DynamoDB). Used by the standalone timer app to:
//!
//! - list events + their disciplines on the paired account,
//! - read a discipline snapshot to seed its local state,
//! - push live timing results when the discipline is in `live` mode.

use axum::Json;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::ApiKeyAuth;
use crate::db::{disciplines, events, runs, users};
use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::{Run, RunStatus, SyncMode};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/me", get(me))
        .route("/me/hmac-key", get(hmac_key))
        .route("/events", get(list_events))
        .route("/disciplines", get(list_disciplines))
        .route("/disciplines/{id}", get(discipline_snapshot))
        .route(
            "/disciplines/{id}/runs/{run_id}/result",
            post(submit_run_result),
        )
}

#[derive(Serialize)]
pub struct DesktopMe {
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub key_id: String,
}

pub async fn me(auth: ApiKeyAuth) -> Json<DesktopMe> {
    Json(DesktopMe {
        sub: auth.user.sub,
        email: auth.user.email,
        display_name: auth.user.display_name,
        key_id: auth.key_id,
    })
}

#[derive(Serialize)]
pub struct HmacKeyResponse {
    pub hmac_key: String,
}

pub async fn hmac_key(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
) -> AppResult<Json<HmacKeyResponse>> {
    let key = users::get_hmac_key(&state.db, &auth.user.sub).await?;
    Ok(Json(HmacKeyResponse { hmac_key: key }))
}

#[derive(Serialize)]
pub struct EventSummary {
    pub id: Uuid,
    pub name: String,
    pub date: chrono::NaiveDate,
    pub location: Option<String>,
    pub is_active: bool,
    pub current_discipline_id: Option<Uuid>,
    /// Legacy mirror of `current_run_ids[0]`.
    pub current_run_id: Option<Uuid>,
    pub current_run_ids: Vec<Uuid>,
}

pub async fn list_events(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
) -> AppResult<Json<Vec<EventSummary>>> {
    let evs = events::list_for_owner(&state.db, &auth.user.sub).await?;
    let summaries = evs
        .into_iter()
        .map(|e| EventSummary {
            id: e.id,
            name: e.name,
            date: e.date,
            location: e.location,
            is_active: e.is_active,
            current_discipline_id: e.current_discipline_id,
            current_run_id: e.current_run_ids.first().copied(),
            current_run_ids: e.current_run_ids,
        })
        .collect();
    Ok(Json(summaries))
}

#[derive(Serialize)]
pub struct DisciplineSummary {
    pub id: Uuid,
    pub event_id: Uuid,
    pub event_name: String,
    pub event_date: chrono::NaiveDate,
    pub name: String,
    pub mode: crate::models::TimerMode,
    pub lane_count: u8,
    pub judge_mode: crate::models::JudgeMode,
    pub sync_mode: SyncMode,
    /// `true` iff this discipline is the one currently selected on the
    /// active event's lane. The desktop usually pairs to this one.
    pub is_current: bool,
}

pub async fn list_disciplines(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
) -> AppResult<Json<Vec<DisciplineSummary>>> {
    let evs = events::list_for_owner(&state.db, &auth.user.sub).await?;
    let mut out: Vec<DisciplineSummary> = Vec::new();
    for ev in evs {
        let disc_list = disciplines::list_for_event(&state.db, ev.id).await?;
        for d in disc_list {
            let is_current = ev.is_active && ev.current_discipline_id == Some(d.id);
            out.push(DisciplineSummary {
                id: d.id,
                event_id: ev.id,
                event_name: ev.name.clone(),
                event_date: ev.date,
                name: d.name,
                mode: d.mode,
                lane_count: d.lane_count,
                judge_mode: d.judge_mode,
                sync_mode: d.sync_mode,
                is_current,
            });
        }
    }
    Ok(Json(out))
}

pub async fn discipline_snapshot(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(id): Path<Uuid>,
) -> AppResult<Json<export::DisciplinePayload>> {
    let payload = export::build_discipline_payload(&state, &auth.user.sub, id).await?;
    Ok(Json(payload))
}

#[derive(Deserialize)]
pub struct SubmitRunResultInput {
    pub run_number: u32,
    pub status: RunStatus,
    pub original_time_ms: Option<u32>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

pub async fn submit_run_result(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((discipline_id, run_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SubmitRunResultInput>,
) -> AppResult<Json<Run>> {
    // Locate the discipline by walking the owner's events. Same trade-off
    // as in `export::build_discipline_payload`; revisit when scale demands.
    let evs = events::list_for_owner(&state.db, &auth.user.sub).await?;
    let mut found = None;
    for ev in &evs {
        if let Some(disc) = disciplines::find(&state.db, ev.id, discipline_id).await? {
            found = Some((ev.clone(), disc));
            break;
        }
    }
    let (event, discipline) =
        found.ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    if discipline.sync_mode != SyncMode::Live {
        return Err(AppError::Conflict(
            "Wettkampf ist im Offline-Modus; Live-Eingabe abgelehnt".into(),
        ));
    }

    let existing = runs::list_for_discipline(&state.db, event.id, discipline.id)
        .await?
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;
    if existing.run_number != input.run_number {
        return Err(AppError::Validation(
            "run_number stimmt nicht mit dem gespeicherten Lauf überein".into(),
        ));
    }

    runs::update_timing(
        &state.db,
        runs::TimingUpdate {
            event_id: event.id,
            discipline_id: discipline.id,
            run_number: existing.run_number,
            run_id: existing.id,
            original_time_ms: input.original_time_ms,
            status: input.status,
            started_at: input.started_at,
            ended_at: input.ended_at,
        },
    )
    .await?;

    let updated = runs::get(
        &state.db,
        event.id,
        discipline.id,
        existing.run_number,
        existing.id,
    )
    .await?
    .ok_or_else(|| AppError::Internal("run vanished mid-update".into()))?;

    Ok(Json(updated))
}
