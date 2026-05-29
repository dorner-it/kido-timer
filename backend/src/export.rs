// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Tamper-evident **discipline-scoped** competition file format used to
//! round-trip data between the web app and the standalone desktop app.
//!
//! The desktop pairs to one discipline at a time and operates on the
//! corresponding `.kido` file. The file carries its parent event's
//! metadata inline so the desktop can show breadcrumbs without making
//! an extra fetch.
//!
//! Wire format (`.kido` file, JSON envelope):
//!
//! ```text
//! {
//!   "v": 3,
//!   "alg": "HS256",
//!   "owner_sub": "<cognito-sub>",
//!   "issued_at": "2026-...",
//!   "payload": "<base64-of-utf8-json-of-DisciplinePayload>",
//!   "sig": "<base64url-no-pad-of-hmac-sha256-of-payload-bytes>"
//! }
//! ```
//!
//! Schema history:
//! - **v1**: original `Competition` flat model.
//! - **v2**: introduced the `Event → Discipline` hierarchy (2026-05).
//! - **v3**: replaced the generic `TimerMode` (`single_lane`,
//!   `two_lane_parallel`, `relay`, `individual`) with the discipline-
//!   specific variants `gruppenstaffette` (2 lanes) and `loeschangriff`
//!   (4 lanes). Existing rows must be backfilled by
//!   `bin/migrate_timer_mode` before this build can serve them.
//!
//! Older envelopes/payloads cannot be imported — clients must update.

use base64::{
    Engine,
    engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::db::{disciplines, events, penalties, runners, runs, team_entries, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Discipline, Event, Penalty, PenaltyStatus, Run, Runner, Team, TeamEntry};
use crate::state::AppState;

pub const SCHEMA_VERSION: u32 = 3;
pub const ENVELOPE_VERSION: u32 = 3;
pub const ALG: &str = "HS256";

type HmacSha256 = Hmac<Sha256>;

/// Event projection without `decider_tokens` (web-only secrets that
/// must never leave the server in an exported file).
#[derive(Serialize, Deserialize)]
pub struct ExportedEvent {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: chrono::NaiveDate,
    pub location: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub current_discipline_id: Option<Uuid>,
    #[serde(default)]
    pub current_run_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ExportedEvent {
    pub fn from_event(e: &Event) -> Self {
        Self {
            id: e.id,
            owner_sub: e.owner_sub.clone(),
            name: e.name.clone(),
            date: e.date,
            location: e.location.clone(),
            is_active: e.is_active,
            current_discipline_id: e.current_discipline_id,
            current_run_id: e.current_run_id,
            created_at: e.created_at,
            updated_at: e.updated_at,
        }
    }
}

// ===== Discipline payload (single-discipline snapshot for desktop) =====

#[derive(Serialize, Deserialize)]
pub struct DisciplinePayload {
    pub schema_version: u32,
    pub exported_at: DateTime<Utc>,
    pub owner_sub: String,
    pub event: ExportedEvent,
    pub discipline: Discipline,
    /// Event-wide teams (for name lookups when the discipline only
    /// references a subset via `team_entries`).
    pub teams: Vec<Team>,
    /// Event-wide runners (relay/individual modes reference these).
    pub runners: Vec<Runner>,
    /// Pre-filtered to this discipline only.
    pub team_entries: Vec<TeamEntry>,
    /// Pre-filtered to this discipline only.
    pub runs: Vec<Run>,
    /// Pre-filtered to this discipline only, status = Approved.
    pub approved_penalties: Vec<Penalty>,
}

// ===== Envelope =====

#[derive(Serialize, Deserialize)]
pub struct Envelope {
    pub v: u32,
    pub alg: String,
    pub owner_sub: String,
    pub issued_at: DateTime<Utc>,
    pub payload: String,
    pub sig: String,
}

// ===== Builders =====

pub async fn build_discipline_payload(
    state: &AppState,
    owner_sub: &str,
    discipline_id: Uuid,
) -> AppResult<DisciplinePayload> {
    // Locate the discipline by scanning the owner's events. This is a
    // small set (a handful of events per user) so it's fine for now;
    // when it grows, add a discipline-id → event-id GSI lookup.
    let evs = events::list_for_owner(&state.db, owner_sub).await?;
    let mut found: Option<(Event, Discipline)> = None;
    for ev in evs {
        if let Some(disc) = disciplines::find(&state.db, ev.id, discipline_id).await? {
            found = Some((ev, disc));
            break;
        }
    }
    let (event, discipline) = found
        .ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    let team_list = teams::list_for_event(&state.db, event.id).await?;
    let runner_list = runners::list_for_event(&state.db, event.id).await?;
    let entry_list =
        team_entries::list_for_discipline(&state.db, event.id, discipline.id).await?;
    let run_list = runs::list_for_discipline(&state.db, event.id, discipline.id).await?;
    let approved: Vec<Penalty> =
        penalties::list_by_status(&state.db, event.id, PenaltyStatus::Approved)
            .await?
            .into_iter()
            .filter(|p| p.discipline_id == discipline.id)
            .collect();

    Ok(DisciplinePayload {
        schema_version: SCHEMA_VERSION,
        exported_at: Utc::now(),
        owner_sub: owner_sub.to_string(),
        event: ExportedEvent::from_event(&event),
        discipline,
        teams: team_list,
        runners: runner_list,
        team_entries: entry_list,
        runs: run_list,
        approved_penalties: approved,
    })
}

// ===== Sign / verify =====

fn decode_hmac_key(b64: &str) -> AppResult<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(b64.as_bytes())
        .map_err(|e| AppError::Internal(format!("hmac key decode: {e}")))
}

fn hmac_sign(key_bytes: &[u8], payload_b64: &str) -> AppResult<String> {
    let mut mac = HmacSha256::new_from_slice(key_bytes)
        .map_err(|e| AppError::Internal(format!("hmac key: {e}")))?;
    mac.update(payload_b64.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

pub fn sign(hmac_key_b64: &str, payload: &DisciplinePayload) -> AppResult<Envelope> {
    let key_bytes = decode_hmac_key(hmac_key_b64)?;
    let payload_json = serde_json::to_vec(payload)
        .map_err(|e| AppError::Internal(format!("serialize payload: {e}")))?;
    let payload_b64 = BASE64.encode(&payload_json);
    let sig = hmac_sign(&key_bytes, &payload_b64)?;

    Ok(Envelope {
        v: ENVELOPE_VERSION,
        alg: ALG.to_string(),
        owner_sub: payload.owner_sub.clone(),
        issued_at: Utc::now(),
        payload: payload_b64,
        sig,
    })
}

pub fn verify(hmac_key_b64: &str, env: &Envelope) -> AppResult<DisciplinePayload> {
    if env.v != ENVELOPE_VERSION {
        return Err(AppError::Validation(format!(
            "Unbekannte Envelope-Version: {} (erwartet {ENVELOPE_VERSION})",
            env.v
        )));
    }
    if env.alg != ALG {
        return Err(AppError::Validation(format!(
            "Unbekannter Algorithmus: {}",
            env.alg
        )));
    }

    let key_bytes = decode_hmac_key(hmac_key_b64)?;
    let expected = hmac_sign(&key_bytes, &env.payload)?;
    let expected_bytes = URL_SAFE_NO_PAD
        .decode(expected.as_bytes())
        .map_err(|e| AppError::Internal(format!("encode our own sig: {e}")))?;

    let provided = URL_SAFE_NO_PAD
        .decode(env.sig.as_bytes())
        .map_err(|_| AppError::Validation("Ungültige Signatur".into()))?;

    if provided.as_slice() != expected_bytes.as_slice() {
        return Err(AppError::Validation(
            "Signatur stimmt nicht. Datei stammt nicht von diesem Konto oder wurde verändert.".into(),
        ));
    }

    let payload_bytes = BASE64
        .decode(env.payload.as_bytes())
        .map_err(|_| AppError::Validation("Payload nicht entschlüsselbar".into()))?;
    let payload: DisciplinePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::Validation("Payload nicht parsebar".into()))?;

    if payload.schema_version != SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "Unbekannte Schema-Version: {}",
            payload.schema_version
        )));
    }

    Ok(payload)
}

// ===== Slug =====

pub fn filename_slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_was_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !out.is_empty() {
            out.push('-');
            last_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("veranstaltung");
    }
    out
}
