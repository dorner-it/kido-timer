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
use crate::db::{disciplines, penalties, runs, team_entries, users};
use crate::error::{AppError, AppResult};
use crate::export;
use crate::models::{Discipline, JudgeMode, PenaltyStatus, SyncMode, TeamEntry, TimerMode};
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

#[derive(Debug, Deserialize)]
pub struct DisciplineInput {
    pub name: String,
    pub mode: TimerMode,
    pub position: u32,
    /// Optional; see [`resolve_lane_count`].
    #[serde(default)]
    pub lane_count: Option<u8>,
    /// Optional; defaults to `per_station` (create) or the stored value
    /// (update).
    #[serde(default)]
    pub judge_mode: Option<JudgeMode>,
}

/// Discipline as returned by `POST`/`PUT`: the stored discipline plus the
/// team ids whose entry lane exceeds `lane_count` (data is never deleted
/// when the lane count shrinks; the UI shows a warning instead).
#[derive(Debug, Serialize)]
pub struct DisciplineResponse {
    #[serde(flatten)]
    pub discipline: Discipline,
    pub lane_conflicts: Vec<Uuid>,
}

/// Validate / default the lane count for `mode`.
///
/// - Explicit value: must be one of `mode.allowed_lane_counts()`
///   (Gruppenstaffette 1 | 2, Löschangriff 4), else `400 validation`.
/// - Omitted on create (`existing = None`): `mode.default_lane_count()`.
/// - Omitted on update: the stored value if it is still valid for the
///   (possibly changed) mode, otherwise the mode default.
pub fn resolve_lane_count(
    mode: TimerMode,
    requested: Option<u8>,
    existing: Option<&Discipline>,
) -> AppResult<u8> {
    let allowed = mode.allowed_lane_counts();
    match requested {
        Some(n) if allowed.contains(&n) => Ok(n),
        Some(n) => Err(AppError::Validation(format!(
            "lane_count {n} ist für {mode:?} nicht erlaubt (erlaubt: {allowed:?})"
        ))),
        None => Ok(existing
            .map(|d| d.lane_count)
            .filter(|n| allowed.contains(n))
            .unwrap_or_else(|| mode.default_lane_count())),
    }
}

/// Team ids whose entry lane is greater than `lane_count`, sorted by
/// start number.
pub fn lane_conflicts(entries: &[TeamEntry], lane_count: u8) -> Vec<Uuid> {
    let mut hits: Vec<&TeamEntry> = entries
        .iter()
        .filter(|e| e.lane.is_some_and(|l| l > lane_count))
        .collect();
    hits.sort_by_key(|e| e.start_number);
    hits.into_iter().map(|e| e.team_id).collect()
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<DisciplineInput>,
) -> AppResult<(StatusCode, Json<DisciplineResponse>)> {
    require_owned(&state, &auth, event_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation(
            "discipline name must not be empty".into(),
        ));
    }
    let lane_count = resolve_lane_count(input.mode, input.lane_count, None)?;
    let now = Utc::now();
    let disc = Discipline {
        id: Uuid::new_v4(),
        event_id,
        name: input.name.trim().to_string(),
        mode: input.mode,
        lane_count,
        judge_mode: input.judge_mode.unwrap_or_default(),
        sync_mode: SyncMode::Live,
        position: input.position,
        created_at: now,
        updated_at: now,
    };
    disciplines::put(&state.db, &disc).await?;
    Ok((
        StatusCode::CREATED,
        Json(DisciplineResponse {
            discipline: disc,
            lane_conflicts: Vec::new(),
        }),
    ))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<DisciplineInput>,
) -> AppResult<Json<DisciplineResponse>> {
    require_owned(&state, &auth, event_id).await?;
    if input.name.trim().is_empty() {
        return Err(AppError::Validation(
            "discipline name must not be empty".into(),
        ));
    }
    let existing = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;
    let lane_count = resolve_lane_count(input.mode, input.lane_count, Some(&existing))?;

    if existing.position != input.position {
        disciplines::delete(&state.db, &existing).await?;
    }

    let disc = Discipline {
        id: discipline_id,
        event_id,
        name: input.name.trim().to_string(),
        mode: input.mode,
        lane_count,
        judge_mode: input.judge_mode.unwrap_or(existing.judge_mode),
        sync_mode: existing.sync_mode,
        position: input.position,
        created_at: existing.created_at,
        updated_at: Utc::now(),
    };
    disciplines::put(&state.db, &disc).await?;

    let entries = team_entries::list_for_discipline(&state.db, event_id, discipline_id).await?;
    let conflicts = lane_conflicts(&entries, disc.lane_count);
    Ok(Json(DisciplineResponse {
        discipline: disc,
        lane_conflicts: conflicts,
    }))
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

    let payload = export::build_discipline_payload(&state, &auth.user.sub, discipline_id).await?;
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

#[cfg(test)]
mod tests {
    //! Contract tests for the `POST /api/events/:id/disciplines` body.
    //!
    //! Full end-to-end routing tests would need a DynamoDB stub (not yet
    //! present in this repo — see `tests/integration.rs` which only
    //! exercises `health_router`). Until that exists, we cover the v3
    //! TimerMode contract at the deserialization boundary, which is
    //! exactly where the handler returns `400 validation`: serde rejects
    //! unknown enum variants before our handler runs, and axum's
    //! `Json<T>` extractor turns that into a 400.
    use super::*;

    #[test]
    fn accepts_gruppenstaffette() {
        let body = serde_json::json!({
            "name": "Gruppenstaffette Männer",
            "mode": "gruppenstaffette",
            "position": 1
        });
        let parsed: DisciplineInput =
            serde_json::from_value(body).expect("v3 body must deserialize");
        assert_eq!(parsed.mode, TimerMode::Gruppenstaffette);
        assert_eq!(parsed.lane_count, None);
        assert_eq!(parsed.judge_mode, None);
    }

    #[test]
    fn accepts_loeschangriff() {
        let body = serde_json::json!({
            "name": "Löschangriff Männer",
            "mode": "loeschangriff",
            "position": 2
        });
        let parsed: DisciplineInput = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.mode, TimerMode::Loeschangriff);
    }

    #[test]
    fn rejects_v1_two_lane_parallel() {
        let body = serde_json::json!({
            "name": "Old Mode",
            "mode": "two_lane_parallel",
            "position": 1
        });
        let err = serde_json::from_value::<DisciplineInput>(body).unwrap_err();
        // Serde reports unknown variants — the route's `Json<T>` extractor
        // surfaces this as `400 validation` via axum's default error path.
        let msg = err.to_string();
        assert!(
            msg.contains("two_lane_parallel") || msg.contains("variant"),
            "expected unknown-variant error, got: {msg}"
        );
    }

    #[test]
    fn rejects_all_legacy_modes() {
        for legacy in ["single_lane", "two_lane_parallel", "relay", "individual"] {
            let body = serde_json::json!({
                "name": "Old",
                "mode": legacy,
                "position": 1
            });
            assert!(
                serde_json::from_value::<DisciplineInput>(body).is_err(),
                "legacy mode {legacy} must be rejected at the JSON boundary"
            );
        }
    }

    #[test]
    fn accepts_lane_count_and_judge_mode() {
        let body = serde_json::json!({
            "name": "GS",
            "mode": "gruppenstaffette",
            "position": 1,
            "lane_count": 1,
            "judge_mode": "per_station_and_lane"
        });
        let parsed: DisciplineInput = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.lane_count, Some(1));
        assert_eq!(parsed.judge_mode, Some(JudgeMode::PerStationAndLane));
    }

    #[test]
    fn rejects_unknown_judge_mode() {
        let body = serde_json::json!({
            "name": "GS",
            "mode": "gruppenstaffette",
            "position": 1,
            "judge_mode": "per_lane"
        });
        assert!(serde_json::from_value::<DisciplineInput>(body).is_err());
    }

    fn disc(mode: TimerMode, lane_count: u8) -> Discipline {
        Discipline {
            id: Uuid::nil(),
            event_id: Uuid::nil(),
            name: "D".into(),
            mode,
            lane_count,
            judge_mode: JudgeMode::PerStation,
            sync_mode: SyncMode::Live,
            position: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn entry(start_number: u32, lane: Option<u8>) -> TeamEntry {
        TeamEntry {
            team_id: Uuid::new_v4(),
            discipline_id: Uuid::nil(),
            event_id: Uuid::nil(),
            start_number,
            lane,
            wg: None,
            wg_history: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn lane_count_gruppenstaffette_accepts_1_and_2() {
        let m = TimerMode::Gruppenstaffette;
        assert_eq!(resolve_lane_count(m, Some(1), None).unwrap(), 1);
        assert_eq!(resolve_lane_count(m, Some(2), None).unwrap(), 2);
    }

    #[test]
    fn lane_count_gruppenstaffette_rejects_others() {
        for n in [0u8, 3, 4, 255] {
            let err = resolve_lane_count(TimerMode::Gruppenstaffette, Some(n), None).unwrap_err();
            assert_eq!(err.code(), "validation", "lane_count {n}");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn lane_count_loeschangriff_only_4() {
        assert_eq!(
            resolve_lane_count(TimerMode::Loeschangriff, Some(4), None).unwrap(),
            4
        );
        for n in [1u8, 2, 3, 5] {
            assert!(resolve_lane_count(TimerMode::Loeschangriff, Some(n), None).is_err());
        }
    }

    #[test]
    fn lane_count_defaults_on_create() {
        assert_eq!(
            resolve_lane_count(TimerMode::Gruppenstaffette, None, None).unwrap(),
            2
        );
        assert_eq!(
            resolve_lane_count(TimerMode::Loeschangriff, None, None).unwrap(),
            4
        );
    }

    #[test]
    fn lane_count_omitted_on_update_keeps_stored_value() {
        let existing = disc(TimerMode::Gruppenstaffette, 1);
        assert_eq!(
            resolve_lane_count(TimerMode::Gruppenstaffette, None, Some(&existing)).unwrap(),
            1
        );
    }

    #[test]
    fn lane_count_omitted_on_mode_change_falls_back_to_default() {
        let existing = disc(TimerMode::Gruppenstaffette, 1);
        assert_eq!(
            resolve_lane_count(TimerMode::Loeschangriff, None, Some(&existing)).unwrap(),
            4
        );
        let existing = disc(TimerMode::Loeschangriff, 4);
        assert_eq!(
            resolve_lane_count(TimerMode::Gruppenstaffette, None, Some(&existing)).unwrap(),
            2
        );
    }

    #[test]
    fn lane_conflicts_lists_entries_above_lane_count() {
        let a = entry(2, Some(2));
        let b = entry(1, Some(1));
        let c = entry(3, None);
        let d = entry(0, Some(2));
        let entries = vec![a.clone(), b, c, d.clone()];
        assert_eq!(lane_conflicts(&entries, 1), vec![d.team_id, a.team_id]);
        assert!(lane_conflicts(&entries, 2).is_empty());
    }

    #[test]
    fn response_flattens_discipline_and_adds_conflicts() {
        let r = DisciplineResponse {
            discipline: disc(TimerMode::Gruppenstaffette, 1),
            lane_conflicts: vec![Uuid::nil()],
        };
        let out = serde_json::to_value(&r).unwrap();
        assert_eq!(out["lane_count"], 1);
        assert_eq!(out["judge_mode"], "per_station");
        assert_eq!(out["mode"], "gruppenstaffette");
        assert_eq!(out["lane_conflicts"], serde_json::json!([Uuid::nil()]));
    }
}
