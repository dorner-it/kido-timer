// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{disciplines, penalties, runs, users};
use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::{Discipline, PenaltyStatus, SyncMode, TimerMode};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/disciplines", post(create))
        .route(
            "/{id}/disciplines/{discipline_id}",
            put(update).delete(remove),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/sync-mode",
            post(set_sync_mode),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/export-file",
            get(export_file),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/import-results",
            post(import_results),
        )
}

#[derive(Deserialize)]
pub struct DisciplineInput {
    pub name: String,
    pub mode: TimerMode,
    pub position: u32,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<DisciplineInput>,
) -> AppResult<(StatusCode, Json<Discipline>)> {
    require_owned(&state, &auth, event_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("discipline name must not be empty".into()));
    }
    let now = Utc::now();
    let disc = Discipline {
        id: Uuid::new_v4(),
        event_id,
        name: input.name.trim().to_string(),
        mode: input.mode,
        sync_mode: SyncMode::Live,
        position: input.position,
        created_at: now,
        updated_at: now,
    };
    disciplines::put(&state.db, &disc).await?;
    Ok((StatusCode::CREATED, Json(disc)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<DisciplineInput>,
) -> AppResult<Json<Discipline>> {
    require_owned(&state, &auth, event_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("discipline name must not be empty".into()));
    }
    let existing = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    if existing.position != input.position {
        disciplines::delete(&state.db, &existing).await?;
    }

    let disc = Discipline {
        id: discipline_id,
        event_id,
        name: input.name.trim().to_string(),
        mode: input.mode,
        sync_mode: existing.sync_mode,
        position: input.position,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    disciplines::put(&state.db, &disc).await?;
    Ok(Json(disc))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, event_id).await?;
    let existing = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;
    disciplines::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct SetSyncModeInput {
    pub sync_mode: SyncMode,
}

pub async fn set_sync_mode(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<SetSyncModeInput>,
) -> AppResult<Json<Discipline>> {
    require_owned(&state, &auth, event_id).await?;
    let existing = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;
    disciplines::set_sync_mode(&state.db, &existing, input.sync_mode).await?;
    let updated = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::Internal("discipline vanished mid-update".into()))?;
    Ok(Json(updated))
}

// ===== export / import (per discipline) =====

pub async fn export_file(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::response::Response> {
    let event = require_owned(&state, &auth, event_id).await?;
    let disc = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    let payload =
        export::build_discipline_payload(&state, &auth.user.sub, discipline_id).await?;
    let hmac_key = users::get_hmac_key(&state.db, &auth.user.sub).await?;
    let envelope = export::sign(&hmac_key, &payload)?;

    let body = serde_json::to_vec(&envelope)
        .map_err(|e| AppError::Internal(format!("serialize envelope: {e}")))?;

    let filename = format!(
        "kido-{}-{}-{}.kido",
        export::filename_slug(&event.name),
        export::filename_slug(&disc.name),
        event.date,
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    let disposition = format!("attachment; filename=\"{filename}\"");
    let disposition_value = HeaderValue::from_str(&disposition)
        .map_err(|e| AppError::Internal(format!("build content-disposition: {e}")))?;
    headers.insert(header::CONTENT_DISPOSITION, disposition_value);

    Ok((headers, body).into_response())
}

#[derive(Serialize)]
pub struct ImportResultsResponse {
    pub runs_imported: usize,
    pub penalties_imported: usize,
    pub runs_skipped: usize,
    pub penalties_skipped: usize,
}

pub async fn import_results(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(envelope): Json<export::Envelope>,
) -> AppResult<Json<ImportResultsResponse>> {
    require_owned(&state, &auth, event_id).await?;
    let disc = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    if disc.sync_mode != SyncMode::Offline {
        return Err(AppError::Conflict(
            "Wettkampf ist nicht im Offline-Modus. Wechseln Sie zuerst in den Offline-Modus, bevor Sie Ergebnisse importieren.".into(),
        ));
    }

    if envelope.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }

    let hmac_key = users::get_hmac_key(&state.db, &auth.user.sub).await?;
    let payload = export::verify(&hmac_key, &envelope)?;

    if payload.discipline.id != discipline_id {
        return Err(AppError::Validation(
            "Datei gehört zu einem anderen Wettkampf".into(),
        ));
    }
    if payload.event.id != event_id {
        return Err(AppError::Validation(
            "Datei gehört zu einer anderen Veranstaltung".into(),
        ));
    }
    if payload.owner_sub != auth.user.sub {
        return Err(AppError::Forbidden);
    }

    let mut runs_imported = 0usize;
    let mut runs_skipped = 0usize;
    for run in payload.runs {
        if run.event_id != event_id || run.discipline_id != discipline_id {
            runs_skipped += 1;
            continue;
        }
        runs::put(&state.db, &run).await?;
        runs_imported += 1;
    }

    let mut penalties_imported = 0usize;
    let mut penalties_skipped = 0usize;
    let now = Utc::now();
    for mut penalty in payload.approved_penalties {
        if penalty.event_id != event_id || penalty.discipline_id != discipline_id {
            penalties_skipped += 1;
            continue;
        }
        penalty.status = PenaltyStatus::Approved;
        if penalty.reviewed_at.is_none() {
            penalty.reviewed_at = Some(now);
        }
        if penalty.reviewed_by.is_none() {
            penalty.reviewed_by = Some("offline-import".to_string());
        }
        penalties::put(&state.db, &penalty).await?;
        penalties_imported += 1;
    }

    Ok(Json(ImportResultsResponse {
        runs_imported,
        penalties_imported,
        runs_skipped,
        penalties_skipped,
    }))
}
