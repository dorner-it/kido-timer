// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Tamper-evident competition file format used to round-trip data between
//! the web app and the standalone desktop app.
//!
//! Wire format (`.kido` file, JSON):
//!
//! ```text
//! {
//!   "v": 1,
//!   "alg": "HS256",
//!   "owner_sub": "<cognito-sub>",
//!   "issued_at": "2026-...",
//!   "payload": "<base64-of-utf8-json-of-CompetitionPayload>",
//!   "sig": "<base64url-no-pad-of-hmac-sha256-of-payload-bytes>"
//! }
//! ```
//!
//! Signature is HMAC-SHA256(payload_bytes) with the **owner's per-user
//! HMAC key**. Files are tenant-scoped: the import endpoint verifies the
//! signature using the *uploading user's* key, so a file from one tenant
//! cannot be imported into another.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::db::{competitions, penalties, runners, runs, teams};
use crate::error::{AppError, AppResult};
use crate::models::{Competition, Penalty, PenaltyStatus, Run, Runner, Team};
use crate::state::AppState;

pub const SCHEMA_VERSION: u32 = 1;
pub const ENVELOPE_VERSION: u32 = 1;
pub const ALG: &str = "HS256";

type HmacSha256 = Hmac<Sha256>;

#[derive(Serialize, Deserialize)]
pub struct CompetitionPayload {
    pub schema_version: u32,
    pub exported_at: DateTime<Utc>,
    pub owner_sub: String,
    pub competition: ExportedCompetition,
    pub teams: Vec<Team>,
    pub runners: Vec<Runner>,
    pub runs: Vec<Run>,
    pub approved_penalties: Vec<Penalty>,
}

/// Competition projection without `decider_tokens` (web-only secrets that
/// should never leave the server in an exported file).
#[derive(Serialize, Deserialize)]
pub struct ExportedCompetition {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: chrono::NaiveDate,
    pub location: Option<String>,
    pub mode: crate::models::TimerMode,
    pub sync_mode: crate::models::SyncMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ExportedCompetition {
    fn from_competition(c: &Competition) -> Self {
        Self {
            id: c.id,
            owner_sub: c.owner_sub.clone(),
            name: c.name.clone(),
            date: c.date,
            location: c.location.clone(),
            mode: c.mode,
            sync_mode: c.sync_mode,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Envelope {
    pub v: u32,
    pub alg: String,
    pub owner_sub: String,
    pub issued_at: DateTime<Utc>,
    pub payload: String,
    pub sig: String,
}

/// Gather the full competition snapshot from DDB.
pub async fn build_payload(
    state: &AppState,
    owner_sub: &str,
    competition_id: Uuid,
) -> AppResult<CompetitionPayload> {
    let comp = competitions::get(&state.db, competition_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("competition {competition_id}")))?;
    if comp.owner_sub != owner_sub {
        return Err(AppError::Forbidden);
    }

    let team_list = teams::list_for_competition(&state.db, competition_id).await?;

    let mut runner_list: Vec<Runner> = Vec::new();
    for team in &team_list {
        let mut team_runners = runners::list_for_team(&state.db, competition_id, team.id).await?;
        runner_list.append(&mut team_runners);
    }

    let run_list = runs::list_for_competition(&state.db, competition_id).await?;
    let approved = penalties::list_by_status(&state.db, competition_id, PenaltyStatus::Approved)
        .await?;

    Ok(CompetitionPayload {
        schema_version: SCHEMA_VERSION,
        exported_at: Utc::now(),
        owner_sub: owner_sub.to_string(),
        competition: ExportedCompetition::from_competition(&comp),
        teams: team_list,
        runners: runner_list,
        runs: run_list,
        approved_penalties: approved,
    })
}

fn decode_hmac_key(b64: &str) -> AppResult<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(b64.as_bytes())
        .map_err(|e| AppError::Internal(format!("hmac key decode: {e}")))
}

/// Wrap a payload in a signed envelope.
pub fn sign(hmac_key_b64: &str, payload: &CompetitionPayload) -> AppResult<Envelope> {
    let key_bytes = decode_hmac_key(hmac_key_b64)?;
    let payload_json = serde_json::to_vec(payload)
        .map_err(|e| AppError::Internal(format!("serialize payload: {e}")))?;
    let payload_b64 = BASE64.encode(&payload_json);

    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .map_err(|e| AppError::Internal(format!("hmac key: {e}")))?;
    mac.update(payload_b64.as_bytes());
    let sig = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

    Ok(Envelope {
        v: ENVELOPE_VERSION,
        alg: ALG.to_string(),
        owner_sub: payload.owner_sub.clone(),
        issued_at: Utc::now(),
        payload: payload_b64,
        sig,
    })
}

/// Verify an envelope and decode its payload. Returns `Validation` on any
/// signature mismatch / shape problem.
pub fn verify(hmac_key_b64: &str, env: &Envelope) -> AppResult<CompetitionPayload> {
    if env.v != ENVELOPE_VERSION {
        return Err(AppError::Validation(format!(
            "Unbekannte Envelope-Version: {}",
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
    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .map_err(|e| AppError::Internal(format!("hmac key: {e}")))?;
    mac.update(env.payload.as_bytes());
    let expected = mac.finalize().into_bytes();

    let provided = URL_SAFE_NO_PAD
        .decode(env.sig.as_bytes())
        .map_err(|_| AppError::Validation("Ungültige Signatur".into()))?;

    if provided.as_slice() != expected.as_slice() {
        return Err(AppError::Validation(
            "Signatur stimmt nicht. Datei stammt nicht von diesem Konto oder wurde verändert."
                .into(),
        ));
    }

    let payload_bytes = BASE64
        .decode(env.payload.as_bytes())
        .map_err(|_| AppError::Validation("Payload nicht entschlüsselbar".into()))?;
    let payload: CompetitionPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::Validation("Payload nicht parsebar".into()))?;

    if payload.schema_version != SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "Unbekannte Schema-Version: {}",
            payload.schema_version
        )));
    }

    Ok(payload)
}

/// Best-effort filename slug for the downloaded file. Lower-case ASCII
/// alphanumerics + hyphens.
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
        out.push_str("wettkampf");
    }
    out
}

