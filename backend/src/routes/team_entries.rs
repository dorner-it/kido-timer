// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete as method_delete, post};
use chrono::{DateTime, Datelike, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::db::{disciplines, runners, team_entries};
use crate::error::{AppError, AppResult};
use crate::models::{TeamEntry, TeamEntryView, User, Wertungsgruppe, WgHistoryEntry};
use crate::routes::events::require_owned;
use crate::state::AppState;

pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/{id}/disciplines/{discipline_id}/entries", post(upsert))
        .route(
            "/{id}/disciplines/{discipline_id}/entries/{team_id}",
            method_delete(remove),
        )
        .route(
            "/{id}/disciplines/{discipline_id}/entries/{team_id}/move-wg",
            post(move_wg),
        )
}

#[derive(Debug, Deserialize)]
pub struct TeamEntryInput {
    pub team_id: Uuid,
    pub start_number: u32,
    pub lane: Option<u8>,
    /// Wertungsgruppe ticked by the club. Required — kept `Option` here
    /// only so a missing value yields `400 validation` (see
    /// [`require_wg`]) instead of axum's generic 422.
    #[serde(default)]
    pub wg: Option<Wertungsgruppe>,
}

/// `wg` is required input on every write (never derived).
pub fn require_wg(wg: Option<Wertungsgruppe>) -> AppResult<Wertungsgruppe> {
    wg.ok_or_else(|| AppError::Validation("wg (Wertungsgruppe) muss angegeben werden".into()))
}

/// Lane must be within `1..=lane_count` of the discipline.
pub fn validate_lane(lane: Option<u8>, lane_count: u8) -> AppResult<()> {
    match lane {
        Some(l) if l == 0 || l > lane_count => Err(AppError::Validation(format!(
            "lane must be between 1 and {lane_count}"
        ))),
        _ => Ok(()),
    }
}

/// Upsert a team's entry in a discipline. If an entry exists with a different
/// start_number, the old row is deleted (start_number is in the SK) before
/// writing the new one. `wg_history` of an existing entry is preserved.
pub async fn upsert(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<TeamEntryInput>,
) -> AppResult<(StatusCode, Json<TeamEntryView>)> {
    let event = require_owned(&state, &auth, event_id).await?;

    let disc = disciplines::find(&state.db, event_id, discipline_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    let wg = require_wg(input.wg)?;
    validate_lane(input.lane, disc.lane_count)?;

    let existing = team_entries::find(&state.db, event_id, discipline_id, input.team_id).await?;

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
        wg_history: existing
            .as_ref()
            .map(|e| e.wg_history.clone())
            .unwrap_or_default(),
        created_at: existing.as_ref().map(|e| e.created_at).unwrap_or(now),
        updated_at: now,
    };
    team_entries::put(&state.db, &entry).await?;

    let roster = runners::list_for_team(&state.db, event_id, input.team_id).await?;
    let code = if existing.is_some() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        code,
        Json(TeamEntryView::new(entry, &roster, event.date.year())),
    ))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id, team_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_owned(&state, &auth, event_id).await?;
    let existing = team_entries::find(&state.db, event_id, discipline_id, team_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("entry for team {team_id}")))?;
    team_entries::delete(&state.db, &existing).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===== Umsetzen (move to another WG) =====

#[derive(Debug, Deserialize)]
pub struct MoveWgInput {
    /// Required (validated in the handler → `400 validation`).
    #[serde(default)]
    pub wg: Option<Wertungsgruppe>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Who is recorded in `wg_history.by`: the organizer's e-mail, falling
/// back to the Cognito `sub`.
pub fn history_actor(user: &User) -> String {
    let email = user.email.trim();
    if email.is_empty() {
        user.sub.clone()
    } else {
        email.to_string()
    }
}

/// Apply an "Umsetzen": set `wg` and append one `wg_history` record.
///
/// Moving to the WG the entry already has is a no-op (nothing is
/// appended) so a double click doesn't pollute the log. Returns whether
/// anything changed.
pub fn apply_move_wg(
    entry: &mut TeamEntry,
    to: Wertungsgruppe,
    by: String,
    note: Option<String>,
    at: DateTime<Utc>,
) -> bool {
    if entry.wg == Some(to) {
        return false;
    }
    entry.wg_history.push(WgHistoryEntry {
        from: entry.wg,
        to,
        by,
        at,
        note: note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
    });
    entry.wg = Some(to);
    entry.updated_at = at;
    true
}

pub async fn move_wg(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((event_id, discipline_id, team_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(input): Json<MoveWgInput>,
) -> AppResult<Json<TeamEntryView>> {
    let event = require_owned(&state, &auth, event_id).await?;
    let to = require_wg(input.wg)?;
    let mut entry = team_entries::find(&state.db, event_id, discipline_id, team_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("entry for team {team_id}")))?;

    if apply_move_wg(
        &mut entry,
        to,
        history_actor(&auth.user),
        input.note,
        Utc::now(),
    ) {
        // Same SK (start_number unchanged) → plain overwrite.
        team_entries::put(&state.db, &entry).await?;
    }

    let roster = runners::list_for_team(&state.db, event_id, team_id).await?;
    Ok(Json(TeamEntryView::new(entry, &roster, event.date.year())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(wg: Option<Wertungsgruppe>) -> TeamEntry {
        TeamEntry {
            team_id: Uuid::new_v4(),
            discipline_id: Uuid::nil(),
            event_id: Uuid::nil(),
            start_number: 1,
            lane: None,
            wg,
            wg_history: Vec::new(),
            created_at: "2027-06-01T00:00:00Z".parse().unwrap(),
            updated_at: "2027-06-01T00:00:00Z".parse().unwrap(),
        }
    }

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn input_without_wg_parses_but_is_rejected() {
        let body = serde_json::json!({ "team_id": Uuid::nil(), "start_number": 1, "lane": null });
        let parsed: TeamEntryInput = serde_json::from_value(body).unwrap();
        let err = require_wg(parsed.wg).unwrap_err();
        assert_eq!(err.code(), "validation");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn input_with_wg_is_accepted() {
        let body = serde_json::json!({
            "team_id": Uuid::nil(), "start_number": 1, "lane": 1, "wg": "wg3"
        });
        let parsed: TeamEntryInput = serde_json::from_value(body).unwrap();
        assert_eq!(require_wg(parsed.wg).unwrap(), Wertungsgruppe::Wg3);
    }

    #[test]
    fn input_with_unknown_wg_is_rejected_by_serde() {
        let body = serde_json::json!({
            "team_id": Uuid::nil(), "start_number": 1, "lane": 1, "wg": "wg7"
        });
        assert!(serde_json::from_value::<TeamEntryInput>(body).is_err());
    }

    #[test]
    fn lane_validation_follows_lane_count() {
        assert!(validate_lane(None, 1).is_ok());
        assert!(validate_lane(Some(1), 1).is_ok());
        assert!(validate_lane(Some(2), 1).is_err());
        assert!(validate_lane(Some(2), 2).is_ok());
        assert!(validate_lane(Some(3), 2).is_err());
        assert!(validate_lane(Some(0), 2).is_err());
        assert!(validate_lane(Some(4), 4).is_ok());
    }

    #[test]
    fn move_wg_sets_wg_and_appends_history() {
        let mut e = entry(Some(Wertungsgruppe::Wg2));
        let changed = apply_move_wg(
            &mut e,
            Wertungsgruppe::Wg1,
            "fbl@example.org".into(),
            Some("  2 Mädchen ".into()),
            at("2027-06-12T09:00:00Z"),
        );
        assert!(changed);
        assert_eq!(e.wg, Some(Wertungsgruppe::Wg1));
        assert_eq!(e.updated_at, at("2027-06-12T09:00:00Z"));
        assert_eq!(
            e.wg_history,
            vec![WgHistoryEntry {
                from: Some(Wertungsgruppe::Wg2),
                to: Wertungsgruppe::Wg1,
                by: "fbl@example.org".into(),
                at: at("2027-06-12T09:00:00Z"),
                note: Some("2 Mädchen".into()),
            }]
        );
    }

    #[test]
    fn move_wg_history_is_append_only() {
        let mut e = entry(Some(Wertungsgruppe::Wg2));
        apply_move_wg(
            &mut e,
            Wertungsgruppe::Wg1,
            "a".into(),
            None,
            at("2027-06-12T09:00:00Z"),
        );
        apply_move_wg(
            &mut e,
            Wertungsgruppe::Wg4,
            "b".into(),
            Some("".into()),
            at("2027-06-12T10:00:00Z"),
        );
        assert_eq!(e.wg, Some(Wertungsgruppe::Wg4));
        assert_eq!(e.wg_history.len(), 2);
        assert_eq!(e.wg_history[0].to, Wertungsgruppe::Wg1);
        assert_eq!(e.wg_history[1].from, Some(Wertungsgruppe::Wg1));
        assert_eq!(e.wg_history[1].to, Wertungsgruppe::Wg4);
        assert_eq!(e.wg_history[1].note, None, "blank note dropped");
    }

    #[test]
    fn move_wg_from_legacy_entry_without_wg() {
        let mut e = entry(None);
        assert!(apply_move_wg(
            &mut e,
            Wertungsgruppe::Wg5,
            "a".into(),
            None,
            Utc::now()
        ));
        assert_eq!(e.wg_history[0].from, None);
        assert_eq!(e.wg, Some(Wertungsgruppe::Wg5));
    }

    #[test]
    fn move_wg_to_same_wg_is_noop() {
        let mut e = entry(Some(Wertungsgruppe::Wg3));
        let before = e.clone();
        assert!(!apply_move_wg(
            &mut e,
            Wertungsgruppe::Wg3,
            "a".into(),
            None,
            Utc::now()
        ));
        assert_eq!(e, before);
    }

    #[test]
    fn move_wg_body_requires_wg() {
        let parsed: MoveWgInput =
            serde_json::from_value(serde_json::json!({ "note": "x" })).unwrap();
        assert!(require_wg(parsed.wg).is_err());
        let parsed: MoveWgInput =
            serde_json::from_value(serde_json::json!({ "wg": "wg1" })).unwrap();
        assert_eq!(parsed.wg, Some(Wertungsgruppe::Wg1));
        assert_eq!(parsed.note, None);
    }

    #[test]
    fn history_actor_prefers_email() {
        let mut u = User {
            sub: "sub-1".into(),
            email: "fbl@example.org".into(),
            display_name: "FBL".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(history_actor(&u), "fbl@example.org");
        u.email = "  ".into();
        assert_eq!(history_actor(&u), "sub-1");
    }
}
