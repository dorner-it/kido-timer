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
//!   "v": 4,
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
//! - **v4**: Gruppenstafette 2027 data model — Discipline `lane_count` /
//!   `judge_mode`, Event `current_run_ids`, Run `lineup*` /
//!   `time_source` / `hand_time_*`, TeamEntry `wg` (input) +
//!   `wg_history`, Penalty `kind` / `code` and canonical station keys.
//!   Additive (PR 6, on-prem): `penalties` = **all** penalties of the
//!   discipline (pending, approved, rejected) next to the unchanged
//!   `approved_penalties`; readers default it to `[]`.
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

pub const SCHEMA_VERSION: u32 = 4;
pub const ENVELOPE_VERSION: u32 = 4;
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
    /// Legacy mirror of `current_run_ids[0]`.
    #[serde(default)]
    pub current_run_id: Option<Uuid>,
    #[serde(default)]
    pub current_run_ids: Vec<Uuid>,
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
            current_run_id: e.current_run_ids.first().copied(),
            current_run_ids: e.current_run_ids.clone(),
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
    /// Pre-filtered to this discipline only, status = Approved. Kept
    /// unchanged for old readers.
    pub approved_penalties: Vec<Penalty>,
    /// All penalties of this discipline (pending, approved, rejected),
    /// ordered by `created_at`. Additive in v4 (PR 6); missing in older
    /// files → `[]`.
    #[serde(default)]
    pub penalties: Vec<Penalty>,
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
    let (event, discipline) =
        found.ok_or_else(|| AppError::NotFound(format!("discipline {discipline_id}")))?;

    let team_list = teams::list_for_event(&state.db, event.id).await?;
    let runner_list = runners::list_for_event(&state.db, event.id).await?;
    let entry_list = team_entries::list_for_discipline(&state.db, event.id, discipline.id).await?;
    let run_list = runs::list_for_discipline(&state.db, event.id, discipline.id).await?;
    let mut all_penalties =
        penalties::list_for_discipline(&state.db, event.id, discipline.id).await?;
    all_penalties.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
    let approved: Vec<Penalty> = all_penalties
        .iter()
        .filter(|p| p.status == PenaltyStatus::Approved)
        .cloned()
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
        penalties: all_penalties,
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

/// Reject anything but the current schema with a message the operator
/// can act on. Applied to both the envelope `v` and the payload
/// `schema_version`.
pub fn check_schema_version(found: u32) -> AppResult<()> {
    if found == SCHEMA_VERSION {
        return Ok(());
    }
    if found < SCHEMA_VERSION {
        return Err(AppError::Validation(format!(
            "Datei hat das veraltete Schema v{found}; diese Version erwartet v{SCHEMA_VERSION} (Gruppenstafette 2027). Bitte Desktop-App aktualisieren und die Datei neu exportieren."
        )));
    }
    Err(AppError::Validation(format!(
        "Unbekannte Schema-Version: v{found} (erwartet v{SCHEMA_VERSION}). Bitte Web-App aktualisieren."
    )))
}

pub fn verify(hmac_key_b64: &str, env: &Envelope) -> AppResult<DisciplinePayload> {
    check_schema_version(env.v)?;
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
            "Signatur stimmt nicht. Datei stammt nicht von diesem Konto oder wurde verändert."
                .into(),
        ));
    }

    let payload_bytes = BASE64
        .decode(env.payload.as_bytes())
        .map_err(|_| AppError::Validation("Payload nicht entschlüsselbar".into()))?;
    // Check the payload version before the full parse so an old file
    // yields the clear version error instead of "nicht parsebar".
    #[derive(Deserialize)]
    struct VersionProbe {
        schema_version: u32,
    }
    let probe: VersionProbe = serde_json::from_slice(&payload_bytes)
        .map_err(|_| AppError::Validation("Payload nicht parsebar".into()))?;
    check_schema_version(probe.schema_version)?;

    let payload: DisciplinePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| AppError::Validation(format!("Payload nicht parsebar: {e}")))?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        JudgeMode, LineupConfirmedBy, PenaltyKind, StationKind, SyncMode, TimeSource, TimerMode,
        Wertungsgruppe, WgHistoryEntry,
    };

    fn key() -> String {
        URL_SAFE_NO_PAD.encode([7u8; 32])
    }

    fn ts() -> DateTime<Utc> {
        "2027-06-12T10:00:00Z".parse().unwrap()
    }

    fn payload() -> DisciplinePayload {
        let event_id = Uuid::new_v4();
        let discipline_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();
        let run_id = Uuid::new_v4();
        let runner_id = Uuid::new_v4();
        let event = Event {
            id: event_id,
            owner_sub: "sub".into(),
            name: "Kreisausscheid".into(),
            date: chrono::NaiveDate::from_ymd_opt(2027, 6, 12).unwrap(),
            location: None,
            is_active: true,
            current_discipline_id: Some(discipline_id),
            current_run_id: Some(run_id),
            current_run_ids: vec![run_id],
            decider_tokens: ["a".into(), "b".into(), "c".into(), "d".into()],
            station_token: "secret".into(),
            created_at: ts(),
            updated_at: ts(),
        };
        let mut p = DisciplinePayload {
            schema_version: SCHEMA_VERSION,
            exported_at: ts(),
            owner_sub: "sub".into(),
            event: ExportedEvent::from_event(&event),
            discipline: Discipline {
                id: discipline_id,
                event_id,
                name: "Gruppenstafette".into(),
                mode: TimerMode::Gruppenstaffette,
                lane_count: 1,
                judge_mode: JudgeMode::PerStationAndLane,
                sync_mode: SyncMode::Offline,
                position: 1,
                created_at: ts(),
                updated_at: ts(),
            },
            teams: vec![Team {
                id: team_id,
                event_id,
                name: "FF Beispiel".into(),
                position: 1,
                created_at: ts(),
                updated_at: ts(),
            }],
            runners: vec![Runner {
                id: runner_id,
                team_id,
                event_id,
                first_name: "Erika".into(),
                last_name: "Mustermann".into(),
                birth_date: chrono::NaiveDate::from_ymd_opt(2014, 4, 12).unwrap(),
                position: 1,
                gender: None,
                created_at: ts(),
                updated_at: ts(),
            }],
            team_entries: vec![TeamEntry {
                team_id,
                discipline_id,
                event_id,
                start_number: 1,
                lane: Some(1),
                wg: Some(Wertungsgruppe::Wg1),
                wg_history: vec![WgHistoryEntry {
                    from: Some(Wertungsgruppe::Wg2),
                    to: Wertungsgruppe::Wg1,
                    by: "fbl@example.org".into(),
                    at: ts(),
                    note: None,
                }],
                created_at: ts(),
                updated_at: ts(),
            }],
            runs: vec![Run {
                id: run_id,
                event_id,
                discipline_id,
                team_id,
                runner_id: None,
                run_number: 1,
                lane: Some(1),
                status: crate::models::RunStatus::Completed,
                original_time_ms: Some(47_240),
                correction_ms: 0,
                started_at: None,
                ended_at: None,
                lineup: vec![runner_id],
                lineup_confirmed_at: Some(ts()),
                lineup_confirmed_by: Some(LineupConfirmedBy::Startlinie),
                time_source: TimeSource::Handstopp,
                hand_time_1_ms: Some(47_230),
                hand_time_2_ms: Some(47_245),
                created_at: ts(),
                updated_at: ts(),
            }],
            approved_penalties: vec![Penalty {
                id: Uuid::new_v4(),
                event_id,
                discipline_id,
                run_id,
                team_id,
                runner_id: None,
                kind: PenaltyKind::Fv,
                code: Some("knotengestell.mehrere_knoten_falsch".into()),
                station: Some(StationKind::Knotengestell),
                is_disqualification: true,
                decider_slot: 0,
                delta_ms: 0,
                reason_code: None,
                status: PenaltyStatus::Approved,
                created_at: ts(),
                reviewed_at: Some(ts()),
                reviewed_by: Some("sub".into()),
            }],
            penalties: Vec::new(),
        };
        // `penalties` = approved + a pending and a rejected one.
        let mut pending = p.approved_penalties[0].clone();
        pending.id = Uuid::new_v4();
        pending.kind = PenaltyKind::Sz;
        pending.is_disqualification = false;
        pending.code = Some("knotengestell.ein_knoten_falsch".into());
        pending.delta_ms = 15_000;
        pending.status = PenaltyStatus::Pending;
        pending.reviewed_at = None;
        pending.reviewed_by = None;
        let mut rejected = pending.clone();
        rejected.id = Uuid::new_v4();
        rejected.status = PenaltyStatus::Rejected;
        rejected.reviewed_at = Some(ts());
        rejected.reviewed_by = Some("sub".into());
        p.penalties = vec![p.approved_penalties[0].clone(), pending, rejected];
        p
    }

    /// Sign arbitrary payload JSON with an arbitrary envelope version
    /// (to fabricate old files).
    fn sign_raw(payload_json: &serde_json::Value, v: u32) -> Envelope {
        let payload_b64 = BASE64.encode(serde_json::to_vec(payload_json).unwrap());
        let key_bytes = decode_hmac_key(&key()).unwrap();
        Envelope {
            v,
            alg: ALG.to_string(),
            owner_sub: "sub".into(),
            issued_at: ts(),
            sig: hmac_sign(&key_bytes, &payload_b64).unwrap(),
            payload: payload_b64,
        }
    }

    #[test]
    fn versions_are_4() {
        assert_eq!(SCHEMA_VERSION, 4);
        assert_eq!(ENVELOPE_VERSION, 4);
    }

    #[test]
    fn v4_roundtrip_keeps_2027_fields() {
        let p = payload();
        let env = sign(&key(), &p).unwrap();
        assert_eq!(env.v, 4);
        let back = verify(&key(), &env).unwrap();
        assert_eq!(back.schema_version, 4);
        assert_eq!(back.discipline, p.discipline);
        assert_eq!(back.runs, p.runs);
        assert_eq!(back.team_entries, p.team_entries);
        assert_eq!(back.approved_penalties, p.approved_penalties);
        assert_eq!(back.penalties, p.penalties);
        assert_eq!(back.event.current_run_ids, p.event.current_run_ids);
        assert_eq!(
            back.event.current_run_id,
            p.event.current_run_ids.first().copied()
        );
    }

    #[test]
    fn payload_json_shape() {
        let json = serde_json::to_value(payload()).unwrap();
        assert_eq!(json["schema_version"], 4);
        assert_eq!(json["discipline"]["lane_count"], 1);
        assert_eq!(json["discipline"]["judge_mode"], "per_station_and_lane");
        assert!(json["event"]["current_run_ids"].is_array());
        assert!(json["event"].get("decider_tokens").is_none());
        assert!(json["event"].get("station_token").is_none());
        let entry = &json["team_entries"][0];
        assert_eq!(entry["wg"], "wg1");
        assert_eq!(entry["wg_history"][0]["from"], "wg2");
        assert!(
            entry.get("wg_warnings").is_none(),
            "warnings are not exported"
        );
        let run = &json["runs"][0];
        assert_eq!(run["time_source"], "handstopp");
        assert_eq!(run["lineup_confirmed_by"], "startlinie");
        let pen = &json["approved_penalties"][0];
        assert_eq!(pen["kind"], "fv");
        assert_eq!(pen["station"], "knotengestell");
        assert_eq!(pen["is_disqualification"], true);
        let all = json["penalties"].as_array().expect("penalties array");
        let statuses: Vec<&str> = all.iter().map(|p| p["status"].as_str().unwrap()).collect();
        assert_eq!(statuses, vec!["approved", "pending", "rejected"]);
    }

    #[test]
    fn v4_file_without_penalties_defaults_to_empty() {
        // Files written before PR 6 carry only `approved_penalties`.
        let mut p = serde_json::to_value(payload()).unwrap();
        p.as_object_mut().unwrap().remove("penalties");
        let env = sign_raw(&p, 4);
        let back = verify(&key(), &env).unwrap();
        assert!(back.penalties.is_empty());
        assert_eq!(back.approved_penalties.len(), 1);
    }

    #[test]
    fn v3_envelope_rejected_with_clear_message() {
        let mut p = serde_json::to_value(payload()).unwrap();
        p["schema_version"] = serde_json::json!(3);
        let env = sign_raw(&p, 3);
        let err = verify(&key(), &env).err().expect("v3 must be rejected");
        assert_eq!(err.code(), "validation");
        let msg = err.to_string();
        assert!(msg.contains("v3") && msg.contains("v4"), "{msg}");
    }

    #[test]
    fn v3_payload_in_v4_envelope_rejected_with_clear_message() {
        // A v3 payload (no 2027 fields at all) in a v4 envelope.
        let mut p = serde_json::to_value(payload()).unwrap();
        p["schema_version"] = serde_json::json!(3);
        p["runs"] = serde_json::json!([]);
        let env = sign_raw(&p, 4);
        let err = verify(&key(), &env)
            .err()
            .expect("v3 payload must be rejected");
        assert_eq!(err.code(), "validation");
        assert!(err.to_string().contains("v3"), "{err}");
    }

    #[test]
    fn tampered_v4_rejected() {
        let mut env = sign(&key(), &payload()).unwrap();
        env.sig = URL_SAFE_NO_PAD.encode([0u8; 32]);
        assert_eq!(verify(&key(), &env).err().unwrap().code(), "validation");
    }

    #[test]
    fn schema_version_check() {
        assert!(check_schema_version(4).is_ok());
        for old in [1, 2, 3] {
            let err = check_schema_version(old).unwrap_err();
            assert!(err.to_string().contains("veraltete"), "{err}");
        }
        assert!(
            check_schema_version(5)
                .unwrap_err()
                .to_string()
                .contains("Unbekannte")
        );
    }

    #[test]
    fn import_of_legacy_shaped_v4_rows_is_lenient() {
        // v4 file whose rows lack the optional 2027 fields (e.g. written
        // by a client that only bumped the version) still parses with
        // read-time defaults.
        let mut p = serde_json::to_value(payload()).unwrap();
        let run = p["runs"][0].as_object_mut().unwrap();
        for k in [
            "lineup",
            "lineup_confirmed_at",
            "lineup_confirmed_by",
            "time_source",
            "hand_time_1_ms",
            "hand_time_2_ms",
        ] {
            run.remove(k);
        }
        let pen = p["approved_penalties"][0].as_object_mut().unwrap();
        pen.remove("kind");
        pen.remove("code");
        let entry = p["team_entries"][0].as_object_mut().unwrap();
        entry.remove("wg_history");
        p["event"]
            .as_object_mut()
            .unwrap()
            .remove("current_run_ids");
        let env = sign_raw(&p, 4);
        let back = verify(&key(), &env).unwrap();
        assert_eq!(back.runs[0].time_source, TimeSource::Lichtschranke);
        assert!(back.runs[0].lineup.is_empty());
        // is_disqualification = true in the file → fv.
        assert_eq!(back.approved_penalties[0].kind, PenaltyKind::Fv);
        assert!(back.team_entries[0].wg_history.is_empty());
        assert!(back.event.current_run_ids.is_empty());
    }
}
