// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{post, put};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::runners;
use crate::error::{AppError, AppResult};
use crate::models::{Gender, Runner};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/teams/{team_id}/runners", post(create))
        .route(
            "/{id}/teams/{team_id}/runners/{runner_id}",
            put(update).delete(remove),
        )
}

#[derive(Deserialize)]
pub struct RunnerInput {
    pub first_name: String,
    pub last_name: String,
    pub birth_date: NaiveDate,
    pub position: u32,
    /// Optional and legacy — no longer written by the UI (the
    /// Wertungsgruppe is ticked by the club, there is no gender logic).
    #[serde(default)]
    pub gender: Option<Gender>,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RunnerInput>,
) -> AppResult<(StatusCode, Json<Runner>)> {
    require_owned(&state, &auth, event_id).await?;
    validate(&input)?;
    let now = Utc::now();
    let runner = Runner {
        id: Uuid::new_v4(),
        team_id,
        event_id,
        first_name: input.first_name.trim().to_string(),
        last_name: input.last_name.trim().to_string(),
        birth_date: input.birth_date,
        position: input.position,
        gender: input.gender,
        created_at: now,
        updated_at: now,
    };
    // The key contains the runner id, so an occupied position is fine:
    // both runners are kept (listed by position, then name).
    runners::put(&state.db, &runner).await?;
    Ok((StatusCode::CREATED, Json(runner)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id, runner_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<RunnerInput>,
) -> AppResult<Json<Runner>> {
    require_owned(&state, &auth, event_id).await?;
    validate(&input)?;

    let stored = runners::list_stored_for_team(&state.db, event_id, team_id).await?;
    let existing = runners::normalize(stored.clone())
        .into_iter()
        .find(|r| r.id == runner_id)
        .ok_or_else(|| AppError::NotFound(format!("runner {runner_id}")))?;

    let now = Utc::now();
    let runner = Runner {
        id: runner_id,
        team_id,
        event_id,
        first_name: input.first_name.trim().to_string(),
        last_name: input.last_name.trim().to_string(),
        birth_date: input.birth_date,
        position: input.position,
        // Prefer the incoming gender; fall back to the previously-stored
        // one so PUT bodies that omit the field don't wipe legacy values.
        gender: input.gender.or(existing.gender),
        created_at: existing.created_at,
        updated_at: now,
    };
    // Moving onto an occupied position swaps: the runner(s) there take
    // this runner's old position. Old/legacy keys are removed and the new
    // ones written in one transaction (see `runners::plan_update`). The
    // response is only the moved runner — the UI reloads the roster.
    let writes = runners::plan_update(&stored, &runner, now)?;
    runners::apply_atomic(&state.db, event_id, &writes).await?;
    Ok(Json(runner))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, team_id, runner_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, event_id).await?;
    let existing = runners::list_for_team(&state.db, event_id, team_id)
        .await?
        .into_iter()
        .find(|r| r.id == runner_id)
        .ok_or_else(|| AppError::NotFound(format!("runner {runner_id}")))?;
    runners::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn validate(input: &RunnerInput) -> AppResult<()> {
    if input.first_name.trim().is_empty() {
        return Err(AppError::Validation("first_name must not be empty".into()));
    }
    if input.last_name.trim().is_empty() {
        return Err(AppError::Validation("last_name must not be empty".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gender_is_optional() {
        let body = serde_json::json!({
            "first_name": "Erika",
            "last_name": "Mustermann",
            "birth_date": "2015-04-12",
            "position": 1
        });
        let input: RunnerInput = serde_json::from_value(body).unwrap();
        assert_eq!(input.gender, None);
        assert!(validate(&input).is_ok());
    }

    #[test]
    fn legacy_gender_still_accepted() {
        let body = serde_json::json!({
            "first_name": "Erika",
            "last_name": "Mustermann",
            "birth_date": "2015-04-12",
            "position": 1,
            "gender": "female"
        });
        let input: RunnerInput = serde_json::from_value(body).unwrap();
        assert_eq!(input.gender, Some(Gender::Female));
    }

    #[test]
    fn names_required() {
        let body = serde_json::json!({
            "first_name": " ",
            "last_name": "Mustermann",
            "birth_date": "2015-04-12",
            "position": 1
        });
        let input: RunnerInput = serde_json::from_value(body).unwrap();
        assert_eq!(validate(&input).unwrap_err().code(), "validation");
    }
}
