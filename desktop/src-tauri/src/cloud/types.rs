use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Disziplin-Bezeichner (Schema v3+). Während des Web-Schema-Rollouts kann
/// das Backend kurzzeitig noch alte v2-Werte liefern; die UI über
/// `modeLabelDE` mappt beide.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimerMode {
    Gruppenstaffette,
    Loeschangriff,
    // Legacy v2-Werte — werden vom alten Backend weiter geliefert, bis die
    // DDB-Migration durchläuft. Nach v3-Rollout entfernen.
    SingleLane,
    TwoLaneParallel,
    Relay,
    Individual,
}

impl TimerMode {
    /// Bahnen, die dieser Modus benutzt (2 bzw. 4). Legacy-Werte werden auf
    /// ihre v3-Entsprechung gemappt. Aktuell vom Rust-Code nicht aufgerufen;
    /// die UI lebt im React-Frontend.
    #[allow(dead_code)]
    pub fn lane_count(self) -> u8 {
        match self {
            TimerMode::Gruppenstaffette
            | TimerMode::SingleLane
            | TimerMode::TwoLaneParallel
            | TimerMode::Relay => 2,
            TimerMode::Loeschangriff | TimerMode::Individual => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
}

/// Feste Wertungsgruppen aus Alter + Geschlecht des Roster.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Wertungsgruppe {
    Wg1,
    Wg2,
    Wg3,
    Wg4,
    Wg5,
}

/// Stationen, an denen Strafrichter Strafen melden können.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StationKind {
    Start,
    Verteiler,
    Strahlrohr,
    Knoten,
    Kuebelspritze,
    Podest,
    Strahlrohrlinie,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Live,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PenaltyStatus {
    Pending,
    Approved,
    Rejected,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSummary {
    pub id: Uuid,
    pub name: String,
    pub date: String,
    pub location: Option<String>,
    pub is_active: bool,
    pub current_discipline_id: Option<Uuid>,
    pub current_run_id: Option<Uuid>,
}

/// Flat list entry returned by `GET /api/export/disciplines`. Each
/// discipline carries its parent event metadata inline so the picker
/// can display them grouped without a second fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisciplineListItem {
    pub id: Uuid,
    pub event_id: Uuid,
    pub event_name: String,
    pub event_date: String,
    pub name: String,
    pub mode: TimerMode,
    pub sync_mode: SyncMode,
    /// True iff this discipline is the one selected on the active
    /// event's lane. The desktop usually pairs to this one.
    pub is_current: bool,
}

/// Event projection embedded in `DisciplinePayload`. Lighter than the
/// full server-side Event row — `decider_tokens` are stripped before
/// crossing the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedEvent {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: String,
    pub location: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub current_discipline_id: Option<Uuid>,
    #[serde(default)]
    pub current_run_id: Option<Uuid>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Gemeinsamer Token für alle Strafrichter dieses Wettkampfs
    /// (Schema v3+).
    #[serde(default)]
    pub station_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discipline {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub mode: TimerMode,
    pub sync_mode: SyncMode,
    pub position: u32,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub position: u32,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamEntry {
    pub team_id: Uuid,
    pub discipline_id: Uuid,
    pub event_id: Uuid,
    pub start_number: u32,
    #[serde(default)]
    pub lane: Option<u8>,
    /// Aus Roster abgeleitete Wertungsgruppe — null bei Altbestand oder
    /// inkonsistentem Roster.
    #[serde(default)]
    pub wg: Option<Wertungsgruppe>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runner {
    pub id: Uuid,
    pub team_id: Uuid,
    pub event_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: String,
    pub position: u32,
    /// Pflichtfeld ab Schema v3 — bleibt optional, weil Altbestand
    /// vor der Migration kein Geschlecht trägt.
    #[serde(default)]
    pub gender: Option<Gender>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub original_time_ms: Option<u32>,
    #[serde(default)]
    pub correction_ms: i32,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Penalty {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub run_id: Uuid,
    pub team_id: Uuid,
    #[serde(default)]
    pub runner_id: Option<Uuid>,
    pub decider_slot: u8,
    pub delta_ms: i32,
    #[serde(default)]
    pub reason_code: Option<String>,
    pub status: PenaltyStatus,
    pub created_at: String,
    #[serde(default)]
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub reviewed_by: Option<String>,
    /// Station, an der die Strafe gemeldet wurde (Schema v3+).
    #[serde(default)]
    pub station: Option<StationKind>,
    /// Bei Löschangriff: Disqualifikation statt Zeitstrafe (Schema v3+).
    #[serde(default)]
    pub is_disqualification: bool,
}

/// Inner payload of a `.kido` envelope (schema v2). Also the response
/// shape of `GET /api/export/disciplines/:id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
