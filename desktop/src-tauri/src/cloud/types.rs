//! Wire types for the desktop ⇄ web contract, **schema v4**
//! (Gruppenstafette 2027, see `docs/contract-pr1-2027.md` and
//! `docs/desktop-integration.md` §2.5).
//!
//! Domain enums come from the shared `kido-core` crate so the desktop
//! accepts exactly the values the backend writes (incl. the legacy station
//! aliases on input). Timestamps stay `String` (RFC 3339 as received) so a
//! payload round-trips unchanged through `.kido` export.
//!
//! Read-time migration mirrors the backend: every v4 addition is optional
//! on input and defaulted (`current_run_ids` ← `[current_run_id]`,
//! `lane_count` ← mode default, `kind` ← `is_disqualification`, …).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use kido_core::{
    JudgeMode, PenaltyKind, PenaltyStatus, StationKind, SyncMode, TimeSource, TimerMode,
    Wertungsgruppe,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

/// Who confirmed a run's lineup (`Run.lineup_confirmed_by`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineupConfirmedBy {
    /// WR 1 at the Startlinie, from the judge app.
    Startlinie,
    /// The organizer, from the web UI.
    Organizer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeResponse {
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacKeyResponse {
    pub hmac_key: String,
}

/// Entry of `GET /api/export/events`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventSummary {
    pub id: Uuid,
    pub name: String,
    pub date: String,
    pub location: Option<String>,
    pub is_active: bool,
    #[serde(default)]
    pub current_discipline_id: Option<Uuid>,
    /// Legacy mirror of `current_run_ids[0]`.
    #[serde(default)]
    pub current_run_id: Option<Uuid>,
    #[serde(default)]
    pub current_run_ids: Vec<Uuid>,
}

/// Flat list entry returned by `GET /api/export/disciplines`. Each
/// discipline carries its parent event metadata inline so the picker
/// can display them grouped without a second fetch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "DisciplineListItemWire")]
pub struct DisciplineListItem {
    pub id: Uuid,
    pub event_id: Uuid,
    pub event_name: String,
    pub event_date: String,
    pub name: String,
    pub mode: TimerMode,
    pub lane_count: u8,
    pub judge_mode: JudgeMode,
    /// Default sync mode set on the web; the desktop may override it.
    pub sync_mode: SyncMode,
    /// True iff this discipline is the one selected on the active
    /// event's lane. The desktop usually pairs to this one.
    pub is_current: bool,
}

#[derive(Deserialize)]
struct DisciplineListItemWire {
    id: Uuid,
    event_id: Uuid,
    event_name: String,
    event_date: String,
    name: String,
    mode: TimerMode,
    #[serde(default)]
    lane_count: Option<u8>,
    #[serde(default)]
    judge_mode: JudgeMode,
    #[serde(default)]
    sync_mode: SyncMode,
    #[serde(default)]
    is_current: bool,
}

impl From<DisciplineListItemWire> for DisciplineListItem {
    fn from(w: DisciplineListItemWire) -> Self {
        Self {
            id: w.id,
            event_id: w.event_id,
            event_name: w.event_name,
            event_date: w.event_date,
            name: w.name,
            mode: w.mode,
            lane_count: w.lane_count.unwrap_or_else(|| w.mode.default_lane_count()),
            judge_mode: w.judge_mode,
            sync_mode: w.sync_mode,
            is_current: w.is_current,
        }
    }
}

/// Event projection embedded in `DisciplinePayload` (no decider tokens).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "ExportedEventWire")]
pub struct ExportedEvent {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: String,
    pub location: Option<String>,
    pub is_active: bool,
    pub current_discipline_id: Option<Uuid>,
    /// Legacy mirror of `current_run_ids[0]`.
    pub current_run_id: Option<Uuid>,
    /// Runs currently being timed — one per active lane.
    pub current_run_ids: Vec<Uuid>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Deserialize)]
struct ExportedEventWire {
    id: Uuid,
    owner_sub: String,
    name: String,
    date: String,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    current_discipline_id: Option<Uuid>,
    #[serde(default)]
    current_run_id: Option<Uuid>,
    #[serde(default)]
    current_run_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

impl From<ExportedEventWire> for ExportedEvent {
    fn from(w: ExportedEventWire) -> Self {
        let current_run_ids = w
            .current_run_ids
            .unwrap_or_else(|| w.current_run_id.into_iter().collect());
        Self {
            id: w.id,
            owner_sub: w.owner_sub,
            name: w.name,
            date: w.date,
            location: w.location,
            is_active: w.is_active,
            current_discipline_id: w.current_discipline_id,
            current_run_id: current_run_ids.first().copied(),
            current_run_ids,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "DisciplineWire")]
pub struct Discipline {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub mode: TimerMode,
    /// Lanes in use: Gruppenstaffette 1 | 2 (default 2), Löschangriff 4.
    pub lane_count: u8,
    pub judge_mode: JudgeMode,
    /// Default set on the web (`offline` = on-prem).
    pub sync_mode: SyncMode,
    pub position: u32,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Deserialize)]
struct DisciplineWire {
    id: Uuid,
    event_id: Uuid,
    name: String,
    mode: TimerMode,
    #[serde(default)]
    lane_count: Option<u8>,
    #[serde(default)]
    judge_mode: JudgeMode,
    #[serde(default)]
    sync_mode: SyncMode,
    #[serde(default)]
    position: u32,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

impl From<DisciplineWire> for Discipline {
    fn from(w: DisciplineWire) -> Self {
        Self {
            id: w.id,
            event_id: w.event_id,
            name: w.name,
            mode: w.mode,
            lane_count: w.lane_count.unwrap_or_else(|| w.mode.default_lane_count()),
            judge_mode: w.judge_mode,
            sync_mode: w.sync_mode,
            position: w.position,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Team {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub position: u32,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Append-only log entry written by "Umsetzen" on the web.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WgHistoryEntry {
    #[serde(default)]
    pub from: Option<Wertungsgruppe>,
    pub to: Wertungsgruppe,
    pub by: String,
    pub at: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamEntry {
    pub team_id: Uuid,
    pub discipline_id: Uuid,
    pub event_id: Uuid,
    pub start_number: u32,
    #[serde(default)]
    pub lane: Option<u8>,
    /// Ticked by the club (input, never derived). `None` on legacy rows
    /// ("WG fehlt") — such teams are left out of the results.
    #[serde(default)]
    pub wg: Option<Wertungsgruppe>,
    #[serde(default)]
    pub wg_history: Vec<WgHistoryEntry>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Runner {
    pub id: Uuid,
    pub team_id: Uuid,
    pub event_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    /// `YYYY-MM-DD`.
    pub birth_date: String,
    #[serde(default)]
    pub position: u32,
    /// Legacy, no longer written by the web (`null` on new rows).
    #[serde(default)]
    pub gender: Option<Gender>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub team_id: Uuid,
    #[serde(default)]
    pub runner_id: Option<Uuid>,
    pub run_number: u32,
    #[serde(default)]
    pub lane: Option<u8>,
    pub status: RunStatus,
    /// Light-barrier time, or the hand-time mean when `time_source =
    /// handstopp`. This is the scoring input.
    #[serde(default)]
    pub original_time_ms: Option<u32>,
    /// **Legacy, display only.** Sum of approved `delta_ms` for old
    /// clients; scoring never reads it.
    #[serde(default)]
    pub correction_ms: i32,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    /// Runner ids who actually ran (from the Startlinie judge).
    #[serde(default)]
    pub lineup: Vec<Uuid>,
    #[serde(default)]
    pub lineup_confirmed_at: Option<String>,
    #[serde(default)]
    pub lineup_confirmed_by: Option<LineupConfirmedBy>,
    #[serde(default)]
    pub time_source: TimeSource,
    #[serde(default)]
    pub hand_time_1_ms: Option<u32>,
    #[serde(default)]
    pub hand_time_2_ms: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "PenaltyWire")]
pub struct Penalty {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub run_id: Uuid,
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,
    /// SZ (+15 s) or FV (run invalid).
    pub kind: PenaltyKind,
    /// Catalogue code (`"<station>.<slug>"`), `None` on legacy rows.
    pub code: Option<String>,
    /// Canonical station key on output (legacy aliases accepted on input).
    pub station: Option<StationKind>,
    /// Output-only mirror of `kind == fv` for old readers.
    pub is_disqualification: bool,
    /// Deprecated slot (1..=4); `0` in the station flow.
    pub decider_slot: u8,
    pub delta_ms: i32,
    pub reason_code: Option<String>,
    pub status: PenaltyStatus,
    pub created_at: Option<String>,
    pub reviewed_at: Option<String>,
    pub reviewed_by: Option<String>,
}

#[derive(Deserialize)]
struct PenaltyWire {
    id: Uuid,
    event_id: Uuid,
    discipline_id: Uuid,
    run_id: Uuid,
    team_id: Uuid,
    #[serde(default)]
    runner_id: Option<Uuid>,
    #[serde(default)]
    kind: Option<PenaltyKind>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    station: Option<StationKind>,
    #[serde(default)]
    is_disqualification: bool,
    #[serde(default)]
    decider_slot: u8,
    #[serde(default)]
    delta_ms: i32,
    #[serde(default)]
    reason_code: Option<String>,
    status: PenaltyStatus,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    reviewed_at: Option<String>,
    #[serde(default)]
    reviewed_by: Option<String>,
}

impl From<PenaltyWire> for Penalty {
    fn from(w: PenaltyWire) -> Self {
        let kind = w.kind.unwrap_or(if w.is_disqualification {
            PenaltyKind::Fv
        } else {
            PenaltyKind::Sz
        });
        Self {
            id: w.id,
            event_id: w.event_id,
            discipline_id: w.discipline_id,
            run_id: w.run_id,
            team_id: w.team_id,
            runner_id: w.runner_id,
            kind,
            code: w.code,
            station: w.station,
            is_disqualification: kind == PenaltyKind::Fv,
            decider_slot: w.decider_slot,
            delta_ms: w.delta_ms,
            reason_code: w.reason_code,
            status: w.status,
            created_at: w.created_at,
            reviewed_at: w.reviewed_at,
            reviewed_by: w.reviewed_by,
        }
    }
}

/// Inner payload of a `.kido` envelope (schema v4). Also the response
/// shape of `GET /api/export/disciplines/:id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisciplinePayload {
    pub schema_version: u32,
    pub exported_at: String,
    pub owner_sub: String,
    pub event: ExportedEvent,
    pub discipline: Discipline,
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub runners: Vec<Runner>,
    #[serde(default)]
    pub team_entries: Vec<TeamEntry>,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default)]
    pub approved_penalties: Vec<Penalty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KidoEnvelope {
    pub v: u32,
    pub alg: String,
    pub owner_sub: String,
    pub issued_at: String,
    pub payload: String,
    pub sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunResultRequest {
    pub run_number: u32,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_time_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudIdentity {
    pub base_url: String,
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedAccount {
    pub base_url: String,
    pub api_key: String,
    pub hmac_key_b64: String,
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub key_id: String,
}

impl PairedAccount {
    pub fn identity(&self) -> CloudIdentity {
        CloudIdentity {
            base_url: self.base_url.clone(),
            sub: self.sub.clone(),
            email: self.email.clone(),
            display_name: self.display_name.clone(),
            key_id: self.key_id.clone(),
        }
    }
}

/// A locally recorded run result that differs from the web's state.
/// Surfaced when switching back to live mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunDivergence {
    pub run_id: Uuid,
    pub run_number: u32,
    pub team_id: Option<Uuid>,
    pub local_status: RunStatus,
    pub local_time_ms: Option<u32>,
    /// `None` when the run no longer exists on the web.
    pub remote_status: Option<RunStatus>,
    pub remote_time_ms: Option<u32>,
}

// Events are short-lived (emitted once), so the large snapshot variant is
// fine unboxed.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CloudEvent {
    Paired {
        identity: CloudIdentity,
    },
    Cleared,
    SnapshotChanged {
        snapshot: DisciplinePayload,
    },
    SnapshotError {
        message: String,
    },
    ResultPosted {
        run_id: Uuid,
        status: RunStatus,
    },
    ResultPostFailed {
        run_id: Uuid,
        message: String,
    },
    FailedPostsChanged {
        failures: Vec<FailedPost>,
    },
    /// Local results differ from the web after switching to live; the
    /// operator decides (upload / discard). Empty list = resolved.
    DivergenceChanged {
        divergences: Vec<RunDivergence>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FailedPost {
    pub run_id: Uuid,
    pub discipline_id: Uuid,
    pub run_number: u32,
    pub status: RunStatus,
    pub original_time_ms: Option<u32>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error: String,
    pub failed_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenKidoResult {
    pub adopted: bool,
    pub payload: DisciplinePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<KidoConflict>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KidoConflict {
    pub current_discipline_id: Uuid,
    pub current_discipline_name: String,
    pub new_discipline_id: Uuid,
    pub new_discipline_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, sample_payload_json, RUN_A1, RUN_B1};
    use serde_json::{json, Value};

    #[test]
    fn parses_v4_payload_from_backend() {
        let p = sample_payload();
        assert_eq!(p.schema_version, 4);
        assert_eq!(p.discipline.mode, TimerMode::Gruppenstaffette);
        assert_eq!(p.discipline.lane_count, 2);
        assert_eq!(p.discipline.judge_mode, JudgeMode::PerStation);
        assert_eq!(p.discipline.sync_mode, SyncMode::Offline);
        assert_eq!(p.event.current_run_ids, vec![id(RUN_A1), id(RUN_B1)]);
        assert_eq!(p.event.current_run_id, Some(id(RUN_A1)));
        assert_eq!(p.team_entries[0].wg, Some(Wertungsgruppe::Wg2));
        assert_eq!(p.team_entries[0].wg_history.len(), 1);
        assert_eq!(p.team_entries[2].wg, None);
        let run = &p.runs[0];
        assert_eq!(run.lineup.len(), 2);
        assert_eq!(run.lineup_confirmed_by, Some(LineupConfirmedBy::Startlinie));
        assert_eq!(run.time_source, TimeSource::Lichtschranke);
        let pen = &p.approved_penalties[0];
        assert_eq!(pen.kind, PenaltyKind::Sz);
        assert_eq!(pen.station, Some(StationKind::Knotengestell));
        assert_eq!(pen.code.as_deref(), Some("knotengestell.ein_knoten_falsch"));
    }

    #[test]
    fn serialization_round_trip_is_lossless() {
        let p = sample_payload();
        let json = serde_json::to_string(&p).unwrap();
        let back: DisciplinePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        // Every field the backend sent is written back (backend import
        // requires e.g. created_at, delta_ms, wg_history).
        let original: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        let written: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(written, original);
    }

    #[test]
    fn legacy_station_aliases_deserialize_canonically() {
        for (legacy, canonical) in [
            ("start", "startlinie"),
            ("verteiler", "verteilerlinie"),
            ("strahlrohr", "strahlrohrlinie"),
            ("knoten", "knotengestell"),
            ("kuebelspritze", "angriffslinie"),
            ("podest", "podest"),
        ] {
            let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
            v["approved_penalties"][0]["station"] = legacy.into();
            let p: DisciplinePayload = serde_json::from_value(v).unwrap();
            let out = serde_json::to_value(&p.approved_penalties[0]).unwrap();
            assert_eq!(out["station"], canonical, "{legacy}");
        }
    }

    #[test]
    fn unknown_station_is_rejected() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        v["approved_penalties"][0]["station"] = "wasserbehaelter".into();
        assert!(serde_json::from_value::<DisciplinePayload>(v).is_err());
    }

    #[test]
    fn legacy_timer_modes_are_rejected() {
        for legacy in ["single_lane", "two_lane_parallel", "relay", "individual"] {
            let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
            v["discipline"]["mode"] = legacy.into();
            assert!(
                serde_json::from_value::<DisciplinePayload>(v).is_err(),
                "{legacy}"
            );
        }
    }

    #[test]
    fn on_prem_alias_reads_as_offline() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        v["discipline"]["sync_mode"] = "on_prem".into();
        let p: DisciplinePayload = serde_json::from_value(v).unwrap();
        assert_eq!(p.discipline.sync_mode, SyncMode::Offline);
        let out = serde_json::to_value(&p.discipline).unwrap();
        assert_eq!(out["sync_mode"], "offline");
    }

    #[test]
    fn penalty_kind_migrates_from_is_disqualification() {
        let base = json!({
            "id": RUN_A1, "event_id": RUN_A1, "discipline_id": RUN_A1,
            "run_id": RUN_A1, "team_id": RUN_A1, "delta_ms": 5000,
            "status": "pending"
        });
        let mut dq = base.clone();
        dq["is_disqualification"] = true.into();
        let p: Penalty = serde_json::from_value(dq).unwrap();
        assert_eq!(p.kind, PenaltyKind::Fv);
        assert!(p.is_disqualification);
        // Defaults for the deprecated / optional fields.
        assert_eq!(p.decider_slot, 0);
        assert_eq!(p.created_at, None);
        assert_eq!(p.code, None);

        let p: Penalty = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(p.kind, PenaltyKind::Sz);
        assert!(!p.is_disqualification);

        // Explicit kind wins; mirror follows kind.
        let mut explicit = base;
        explicit["kind"] = "fv".into();
        explicit["is_disqualification"] = false.into();
        let p: Penalty = serde_json::from_value(explicit).unwrap();
        assert_eq!(p.kind, PenaltyKind::Fv);
        assert!(p.is_disqualification);
    }

    #[test]
    fn current_run_ids_migrate_from_legacy_field() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        v["event"]
            .as_object_mut()
            .unwrap()
            .remove("current_run_ids");
        let p: DisciplinePayload = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(p.event.current_run_ids, vec![id(RUN_A1)]);

        v["event"]["current_run_id"] = Value::Null;
        let p: DisciplinePayload = serde_json::from_value(v).unwrap();
        assert!(p.event.current_run_ids.is_empty());
        assert_eq!(p.event.current_run_id, None);
    }

    #[test]
    fn current_run_id_mirror_follows_list() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        v["event"]["current_run_id"] = RUN_A1.into();
        v["event"]["current_run_ids"] = json!([RUN_B1]);
        let p: DisciplinePayload = serde_json::from_value(v).unwrap();
        assert_eq!(p.event.current_run_id, Some(id(RUN_B1)));
    }

    #[test]
    fn discipline_defaults_lane_count_and_judge_mode() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        let d = v["discipline"].as_object_mut().unwrap();
        d.remove("lane_count");
        d.remove("judge_mode");
        d["mode"] = "loeschangriff".into();
        let p: DisciplinePayload = serde_json::from_value(v).unwrap();
        assert_eq!(p.discipline.lane_count, 4);
        assert_eq!(p.discipline.judge_mode, JudgeMode::PerStation);
    }

    #[test]
    fn run_defaults_for_v4_fields() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        let r = v["runs"][0].as_object_mut().unwrap();
        for k in [
            "lineup",
            "lineup_confirmed_at",
            "lineup_confirmed_by",
            "time_source",
            "hand_time_1_ms",
            "hand_time_2_ms",
        ] {
            r.remove(k);
        }
        let p: DisciplinePayload = serde_json::from_value(v).unwrap();
        assert!(p.runs[0].lineup.is_empty());
        assert_eq!(p.runs[0].time_source, TimeSource::Lichtschranke);
        assert_eq!(p.runs[0].hand_time_1_ms, None);
    }

    #[test]
    fn handstopp_run_parses() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        v["runs"][0]["time_source"] = "handstopp".into();
        v["runs"][0]["hand_time_1_ms"] = 47_230.into();
        v["runs"][0]["hand_time_2_ms"] = 47_245.into();
        let p: DisciplinePayload = serde_json::from_value(v).unwrap();
        assert_eq!(p.runs[0].time_source, TimeSource::Handstopp);
        assert_eq!(p.runs[0].hand_time_2_ms, Some(47_245));
    }

    #[test]
    fn discipline_list_item_v4_and_defaults() {
        let item: DisciplineListItem = serde_json::from_value(json!({
            "id": RUN_A1, "event_id": RUN_A1, "event_name": "E", "event_date": "2027-06-12",
            "name": "GS", "mode": "gruppenstaffette", "lane_count": 1,
            "judge_mode": "per_station_and_lane", "sync_mode": "live", "is_current": true
        }))
        .unwrap();
        assert_eq!(item.lane_count, 1);
        assert_eq!(item.judge_mode, JudgeMode::PerStationAndLane);

        let item: DisciplineListItem = serde_json::from_value(json!({
            "id": RUN_A1, "event_id": RUN_A1, "event_name": "E", "event_date": "2027-06-12",
            "name": "GS", "mode": "gruppenstaffette", "sync_mode": "offline", "is_current": false
        }))
        .unwrap();
        assert_eq!(item.lane_count, 2);
        assert_eq!(item.judge_mode, JudgeMode::PerStation);
        let out = serde_json::to_value(&item).unwrap();
        assert_eq!(out["lane_count"], 2);
    }

    #[test]
    fn event_summary_v4() {
        let e: EventSummary = serde_json::from_value(json!({
            "id": RUN_A1, "name": "E", "date": "2027-06-12", "location": null,
            "is_active": true, "current_discipline_id": null,
            "current_run_id": RUN_A1, "current_run_ids": [RUN_A1, RUN_B1]
        }))
        .unwrap();
        assert_eq!(e.current_run_ids.len(), 2);
    }

    #[test]
    fn unknown_wg_is_rejected() {
        let mut v: Value = serde_json::from_str(&sample_payload_json()).unwrap();
        v["team_entries"][0]["wg"] = "wg6".into();
        assert!(serde_json::from_value::<DisciplinePayload>(v).is_err());
    }

    #[test]
    fn divergence_event_serializes_camel_case() {
        let ev = CloudEvent::DivergenceChanged {
            divergences: vec![RunDivergence {
                run_id: id(RUN_A1),
                run_number: 1,
                team_id: None,
                local_status: RunStatus::Completed,
                local_time_ms: Some(1),
                remote_status: None,
                remote_time_ms: None,
            }],
        };
        let v = serde_json::to_value(ev).unwrap();
        assert_eq!(v["type"], "divergenceChanged");
        assert_eq!(v["divergences"][0]["localStatus"], "completed");
        assert_eq!(v["divergences"][0]["runNumber"], 1);
    }
}
