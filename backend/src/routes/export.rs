// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Desktop-facing API mounted at `/api/export/...`.
//!
//! Authenticated via the `X-API-Key` header (per-user, argon2id-hashed
//! in DynamoDB). Used by the standalone timer app to:
//!
//! - list events + their disciplines on the paired account,
//! - read a discipline snapshot to seed its local state,
//! - push live timing results when the discipline is in `live` mode,
//! - switch the discipline's `sync_mode` (desktop override, PR 6),
//! - upload everything recorded on-prem in one idempotent call (PR 6).

use axum::Json;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::ApiKeyAuth;
use crate::db::{disciplines, events, penalties, runners, runs, users};
use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::{Discipline, Event, Run, RunStatus, SyncMode};
use crate::onprem::{self, OnpremUploadInput, OnpremUploadResponse};
use crate::routes::disciplines::SetSyncModeInput;
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
        .route("/disciplines/{id}/sync-mode", post(set_sync_mode))
        .route("/disciplines/{id}/onprem-upload", post(onprem_upload))
}

/// Locate a discipline among the API-key owner's events. `404` when it
/// does not exist or belongs to someone else (no existence leak).
///
/// Walks the owner's events (a handful per user); revisit with a
/// discipline-id → event-id GSI lookup when scale demands.
pub async fn find_owned_discipline(
    state: &AppState,
    owner_sub: &str,
    discipline_id: Uuid,
) -> AppResult<(Event, Discipline)> {
    let evs = events::list_for_owner(&state.db, owner_sub).await?;
    for ev in evs {
        if let Some(disc) = disciplines::find(&state.db, ev.id, discipline_id).await? {
            return Ok((ev, disc));
        }
    }
    Err(AppError::NotFound(format!("discipline {discipline_id}")))
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
    let (event, discipline) = find_owned_discipline(&state, &auth.user.sub, discipline_id).await?;

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

/// `POST /api/export/disciplines/{id}/sync-mode` — the desktop switches
/// the web's sync mode (same semantics as the organizer route
/// `POST /api/events/{id}/disciplines/{did}/sync-mode`).
pub async fn set_sync_mode(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(discipline_id): Path<Uuid>,
    Json(input): Json<SetSyncModeInput>,
) -> AppResult<Json<Discipline>> {
    let (event, discipline) = find_owned_discipline(&state, &auth.user.sub, discipline_id).await?;
    disciplines::set_sync_mode(&state.db, &discipline, input.sync_mode).await?;
    let updated = disciplines::find(&state.db, event.id, discipline_id)
        .await?
        .ok_or_else(|| AppError::Internal("discipline vanished mid-update".into()))?;
    tracing::info!(
        discipline_id = %discipline_id,
        sync_mode = ?updated.sync_mode,
        "sync mode set by desktop"
    );
    Ok(Json(updated))
}

/// `POST /api/export/disciplines/{id}/onprem-upload` — everything the
/// desktop recorded on-prem (changed runs, all penalties, current runs)
/// in one idempotent call. Allowed regardless of `sync_mode`.
///
/// All rows are validated first ([`onprem::plan_upload`]); an invalid row
/// fails the whole upload with `400` before anything is written. Only
/// rows that actually change are written, so a repeated upload is a
/// no-op.
pub async fn onprem_upload(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(discipline_id): Path<Uuid>,
    Json(input): Json<OnpremUploadInput>,
) -> AppResult<Json<OnpremUploadResponse>> {
    let (event, discipline) = find_owned_discipline(&state, &auth.user.sub, discipline_id).await?;

    let run_list = runs::list_for_discipline(&state.db, event.id, discipline.id).await?;
    let penalty_list = penalties::list_for_event(&state.db, event.id).await?;
    let roster = runners::list_for_event(&state.db, event.id).await?;

    let plan = onprem::plan_upload(
        &discipline,
        &run_list,
        &penalty_list,
        &roster,
        (event.current_discipline_id, &event.current_run_ids),
        &input,
        chrono::Utc::now(),
    )?;

    for id in &plan.review_conflicts {
        tracing::warn!(
            penalty_id = %id,
            discipline_id = %discipline_id,
            "onprem-upload: review differs from the web (approved vs rejected); kept the web's value"
        );
    }

    // Penalties first, runs (with the recomputed legacy correction_ms)
    // after: a retry after a partial failure converges to the same state.
    for p in &plan.penalties_to_put {
        penalties::put(&state.db, p).await?;
    }
    for r in &plan.runs_to_put {
        runs::put(&state.db, r).await?;
    }
    if let Some(ids) = &plan.current_run_ids {
        events::set_current(&state.db, event.id, Some(discipline.id), ids).await?;
    }

    tracing::info!(
        discipline_id = %discipline_id,
        runs_updated = plan.response.runs_updated,
        penalties_upserted = plan.response.penalties_upserted,
        rows_written = plan.runs_to_put.len() + plan.penalties_to_put.len(),
        "onprem-upload applied"
    );
    Ok(Json(plan.response))
}
