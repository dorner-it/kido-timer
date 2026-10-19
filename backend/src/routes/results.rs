// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! `GET /api/events/{id}/disciplines/{discipline_id}/results` — WG
//! rankings + Tagessieger of one discipline (organizer only). All rules
//! are in `crate::results` / `kido_core::scoring`; this handler only
//! loads the rows.

use axum::Json;
use axum::extract::{Path, State};
use axum::routing::get;
use chrono::Utc;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{disciplines, penalties, runners, runs, team_entries, teams};
use crate::error::{AppError, AppResult};
use crate::results::{ResultsResponse, build_results_response};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new().route(
        "/{id}/disciplines/{discipline_id}/results",
        get(get_results),
    )
}

pub async fn get_results(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<ResultsResponse>> {
    let event = require_owned(&state, &auth, event_id).await?;
    let db = &state.db;
    let (discipline, entries, team_rows, runner_rows, run_rows, penalty_rows) = tokio::try_join!(
        disciplines::find(db, event_id, discipline_id),
        team_entries::list_for_discipline(db, event_id, discipline_id),
        teams::list_for_event(db, event_id),
        runners::list_for_event(db, event_id),
        runs::list_for_discipline(db, event_id, discipline_id),
        penalties::list_for_discipline(db, event_id, discipline_id),
    )?;
    let discipline =
        discipline.ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    Ok(Json(build_results_response(
        &event,
        &discipline,
        &entries,
        &team_rows,
        &runner_rows,
        &run_rows,
        &penalty_rows,
        Utc::now(),
    )))
}
