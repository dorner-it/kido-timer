// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Desktop-facing API mounted at `/api/export/...`.
//!
//! Authenticated via the `X-API-Key` header (per-user, argon2id-hashed
//! in DynamoDB). Used by the standalone timer app to:
//!
//! - read a competition snapshot to seed its local state,
//! - push live timing results when the competition is in `live` mode.

use axum::Json;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::ApiKeyAuth;
use crate::db::{competitions, runs, users};
use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::{Run, RunStatus, SyncMode};
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/me", get(me))
        .route("/me/hmac-key", get(hmac_key))
        .route("/competitions", get(list_competitions))
        .route("/competitions/{id}", get(competition_snapshot))
        .route(
            "/competitions/{id}/runs/{run_id}/result",
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

/// Sanity / who-am-I check for the desktop app. Returns the user
/// identity the API key is bound to, plus the key's row id.
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
    /// Base64url (no padding) encoded 32-byte HMAC-SHA256 key. Required
    /// to sign offline result files before re-uploading them via
    /// `POST /api/competitions/:id/import-results`.
    pub hmac_key: String,
}

/// Returns the per-user HMAC signing key. The desktop app fetches this
/// once after the operator pairs an API key, caches it locally
/// (e.g. OS keychain), and uses it to sign offline `.kido` files it
/// produces. Rotating the user's key (TBD) invalidates outstanding
/// files; the desktop must re-fetch.
pub async fn hmac_key(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
) -> AppResult<Json<HmacKeyResponse>> {
    let key = users::get_hmac_key(&state.db, &auth.user.sub).await?;
    Ok(Json(HmacKeyResponse { hmac_key: key }))
}

#[derive(Serialize)]
pub struct CompetitionSummary {
    pub id: Uuid,
    pub name: String,
    pub date: chrono::NaiveDate,
    pub mode: crate::models::TimerMode,
    pub sync_mode: SyncMode,
    pub is_active: bool,
    pub current_run_id: Option<Uuid>,
}

pub async fn list_competitions(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
) -> AppResult<Json<Vec<CompetitionSummary>>> {
    let comps = competitions::list_for_owner(&state.db, &auth.user.sub).await?;
    let summaries = comps
        .into_iter()
        .map(|c| CompetitionSummary {
            id: c.id,
            name: c.name,
            date: c.date,
            mode: c.mode,
            sync_mode: c.sync_mode,
            is_active: c.is_active,
            current_run_id: c.current_run_id,
        })
        .collect();
    Ok(Json(summaries))
}

/// Full competition snapshot (`schema_version: 1`). The desktop app polls
/// this in `live` mode to keep its local view fresh, and reads it once
/// at the start of an offline run before the operator triggers download
/// of the signed file.
pub async fn competition_snapshot(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path(id): Path<Uuid>,
) -> AppResult<Json<export::CompetitionPayload>> {
    let comp = competitions::get(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("competition {id}")))?;
    if comp.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }
    let payload = export::build_payload(&state, &auth.user.sub, id).await?;
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

/// Idempotent live ingest: the desktop posts this whenever a run's timing
/// state changes (start, finish, cancel). Server preserves
/// `correction_ms`, team/runner/lane assignment.
pub async fn submit_run_result(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    Path((competition_id, run_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SubmitRunResultInput>,
) -> AppResult<Json<Run>> {
    let comp = competitions::get(&state.db, competition_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("competition {competition_id}")))?;
    if comp.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }
    if comp.sync_mode != SyncMode::Live {
        return Err(AppError::Conflict(
            "Wettkampf ist im Offline-Modus; Live-Eingabe abgelehnt".into(),
        ));
    }

    let existing = runs::list_for_competition(&state.db, competition_id)
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
            competition_id,
            run_number: existing.run_number,
            run_id: existing.id,
            original_time_ms: input.original_time_ms,
            status: input.status,
            started_at: input.started_at,
            ended_at: input.ended_at,
        },
    )
    .await?;

    let updated = runs::get(&state.db, competition_id, existing.run_number, existing.id)
        .await?
        .ok_or_else(|| AppError::Internal("run vanished mid-update".into()))?;

    Ok(Json(updated))
}
