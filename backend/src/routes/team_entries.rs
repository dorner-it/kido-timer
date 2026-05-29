// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete as method_delete, post};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{disciplines, runners, team_entries};
use crate::error::{AppError, AppResult};
use crate::models::{TeamEntry, WgError, Wertungsgruppe};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route(
            "/{id}/disciplines/{discipline_id}/entries",
            post(upsert),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/entries/{team_id}",
            method_delete(remove),
        )
}

#[derive(Deserialize)]
pub struct TeamEntryInput {
    pub team_id: Uuid,
    pub start_number: u32,
    pub lane: Option<u8>,
}

/// Upsert a team's entry in a discipline. If an entry exists with a different
/// start_number, the old row is deleted (start_number is in the SK) before
/// writing the new one.
pub async fn upsert(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<TeamEntryInput>,
) -> AppResult<(StatusCode, Json<TeamEntry>)> {
    let event = require_owned(&state, &auth, event_id).await?;

    let _disc = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    if let Some(lane) = input.lane {
        if !(1..=2).contains(&lane) {
            return Err(AppError::Validation("lane must be 1 or 2".into()));
        }
    }

    // Derive WG from the team's current roster against the event date
    // **before** any mutation, so an inconsistent roster fails fast
    // without orphaning the previously-stored entry. The stamp is
    // persisted; later roster edits don't recompute it (re-POST the
    // entry to refresh).
    let roster = runners::list_for_team(&state.db, event_id, input.team_id).await?;
    let wg = match Wertungsgruppe::derive(&roster, event.date) {
        Ok(w) => w,
        Err(WgError::Inconsistent { reason }) => {
            return Err(AppError::WgInconsistent(reason));
        }
    };

    // If an entry exists for (team_id, discipline_id), it may have a
    // different start_number — remove it before writing the new one.
    let existing = team_entries::list_for_discipline(&state.db, event_id, discipline_id)
        .await?
        .into_iter()
        .find(|e| e.team_id == input.team_id);

    if let Some(old) = existing.as_ref() {
        team_entries::delete(&state.db, old).await?;
    }

    let now = Utc::now();
    let entry = TeamEntry {
        team_id: input.team_id,
        discipline_id,
        event_id,
        start_number: input.start_number,
        lane: input.lane,
        wg: Some(wg),
        created_at: existing.as_ref().map(|e| e.created_at).unwrap_or(now),
        updated_at: now,
    };
    team_entries::put(&state.db, &entry).await?;
    let code = if existing.is_some() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((code, Json(entry)))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id, team_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, event_id).await?;
    let existing = team_entries::list_for_discipline(&state.db, event_id, discipline_id)
        .await?
        .into_iter()
        .find(|e| e.team_id == team_id)
        .ok_or_else(|| AppError::NotFound(format!("entry for team {team_id}")))?;
    team_entries::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}
