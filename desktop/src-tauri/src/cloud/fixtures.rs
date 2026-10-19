//! Test fixtures: a schema-v4 discipline payload exactly as the backend
//! (`backend/src/export.rs`) serializes it (incl. `penalties`, contract A1).

use uuid::Uuid;

use super::types::DisciplinePayload;

pub const EVENT: &str = "00000000-0000-4000-8000-000000000001";
pub const DISCIPLINE: &str = "00000000-0000-4000-8000-000000000002";
pub const TEAM_A: &str = "00000000-0000-4000-8000-0000000000a1";
pub const TEAM_B: &str = "00000000-0000-4000-8000-0000000000b1";
pub const TEAM_C: &str = "00000000-0000-4000-8000-0000000000c1";
pub const RUNNER_A1: &str = "00000000-0000-4000-8000-0000000001a1";
pub const RUNNER_A2: &str = "00000000-0000-4000-8000-0000000001a2";
pub const RUNNER_B1: &str = "00000000-0000-4000-8000-0000000001b1";
pub const RUNNER_C1: &str = "00000000-0000-4000-8000-0000000001c1";
pub const RUN_A1: &str = "00000000-0000-4000-8000-00000000a001";
pub const RUN_B1: &str = "00000000-0000-4000-8000-00000000b001";
pub const RUN_C1: &str = "00000000-0000-4000-8000-00000000c001";
pub const PENALTY_1: &str = "00000000-0000-4000-8000-00000000f001";

pub fn id(s: &str) -> Uuid {
    s.parse().unwrap()
}

/// v4 payload: Gruppenstafette, 2 lanes, teams A (WG2) / B (WG2) / C (no
/// WG), runs A1 (lane 1) and B1 (lane 2) current, C1 pending without lane.
pub fn sample_payload_json() -> String {
    format!(
        r#"{{
  "schema_version": 4,
  "exported_at": "2027-06-12T09:00:00Z",
  "owner_sub": "sub-1",
  "event": {{
    "id": "{EVENT}", "owner_sub": "sub-1", "name": "Kreisausscheid",
    "date": "2027-06-12", "location": "Sportplatz", "is_active": true,
    "current_discipline_id": "{DISCIPLINE}",
    "current_run_id": "{RUN_A1}",
    "current_run_ids": ["{RUN_A1}", "{RUN_B1}"],
    "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z"
  }},
  "discipline": {{
    "id": "{DISCIPLINE}", "event_id": "{EVENT}", "name": "Gruppenstafette",
    "mode": "gruppenstaffette", "lane_count": 2, "judge_mode": "per_station",
    "sync_mode": "offline", "position": 1,
    "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z"
  }},
  "teams": [
    {{ "id": "{TEAM_A}", "event_id": "{EVENT}", "name": "FF Alpha", "position": 1,
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{TEAM_B}", "event_id": "{EVENT}", "name": "FF Beta", "position": 2,
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{TEAM_C}", "event_id": "{EVENT}", "name": "FF Gamma", "position": 3,
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }}
  ],
  "runners": [
    {{ "id": "{RUNNER_A1}", "team_id": "{TEAM_A}", "event_id": "{EVENT}",
       "first_name": "Erika", "last_name": "A", "birth_date": "2015-06-12", "position": 1,
       "gender": null, "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{RUNNER_A2}", "team_id": "{TEAM_A}", "event_id": "{EVENT}",
       "first_name": "Max", "last_name": "A", "birth_date": "2013-06-13", "position": 2,
       "gender": null, "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{RUNNER_B1}", "team_id": "{TEAM_B}", "event_id": "{EVENT}",
       "first_name": "Bea", "last_name": "B", "birth_date": "2015-01-01", "position": 1,
       "gender": null, "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{RUNNER_C1}", "team_id": "{TEAM_C}", "event_id": "{EVENT}",
       "first_name": "Cem", "last_name": "C", "birth_date": "2014-01-01", "position": 1,
       "gender": null, "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }}
  ],
  "team_entries": [
    {{ "team_id": "{TEAM_A}", "discipline_id": "{DISCIPLINE}", "event_id": "{EVENT}",
       "start_number": 1, "lane": 1, "wg": "wg2",
       "wg_history": [{{ "from": "wg3", "to": "wg2", "by": "fbl@example.org",
                        "at": "2027-06-12T08:00:00Z", "note": null }}],
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "team_id": "{TEAM_B}", "discipline_id": "{DISCIPLINE}", "event_id": "{EVENT}",
       "start_number": 2, "lane": 2, "wg": "wg2", "wg_history": [],
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "team_id": "{TEAM_C}", "discipline_id": "{DISCIPLINE}", "event_id": "{EVENT}",
       "start_number": 3, "lane": null, "wg": null, "wg_history": [],
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }}
  ],
  "runs": [
    {{ "id": "{RUN_A1}", "event_id": "{EVENT}", "discipline_id": "{DISCIPLINE}",
       "team_id": "{TEAM_A}", "runner_id": null, "run_number": 1, "lane": 1,
       "status": "pending", "original_time_ms": null, "correction_ms": 0,
       "started_at": null, "ended_at": null,
       "lineup": ["{RUNNER_A1}", "{RUNNER_A2}"],
       "lineup_confirmed_at": "2027-06-12T09:30:00Z", "lineup_confirmed_by": "startlinie",
       "time_source": "lichtschranke", "hand_time_1_ms": null, "hand_time_2_ms": null,
       "penalties_accepted_at": null,
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{RUN_B1}", "event_id": "{EVENT}", "discipline_id": "{DISCIPLINE}",
       "team_id": "{TEAM_B}", "runner_id": null, "run_number": 2, "lane": 2,
       "status": "pending", "original_time_ms": null, "correction_ms": 0,
       "started_at": null, "ended_at": null, "lineup": [],
       "lineup_confirmed_at": null, "lineup_confirmed_by": null,
       "time_source": "lichtschranke", "hand_time_1_ms": null, "hand_time_2_ms": null,
       "penalties_accepted_at": null,
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }},
    {{ "id": "{RUN_C1}", "event_id": "{EVENT}", "discipline_id": "{DISCIPLINE}",
       "team_id": "{TEAM_C}", "runner_id": null, "run_number": 3, "lane": null,
       "status": "pending", "original_time_ms": null, "correction_ms": 0,
       "started_at": null, "ended_at": null, "lineup": [],
       "lineup_confirmed_at": null, "lineup_confirmed_by": null,
       "time_source": "lichtschranke", "hand_time_1_ms": null, "hand_time_2_ms": null,
       "penalties_accepted_at": null,
       "created_at": "2027-01-01T00:00:00Z", "updated_at": "2027-01-01T00:00:00Z" }}
  ],
  "approved_penalties": [
    {{ "id": "{PENALTY_1}", "event_id": "{EVENT}", "discipline_id": "{DISCIPLINE}",
       "run_id": "{RUN_A1}", "team_id": "{TEAM_A}", "runner_id": null,
       "kind": "sz", "code": "knotengestell.ein_knoten_falsch", "station": "knotengestell",
       "is_disqualification": false, "decider_slot": 0, "delta_ms": 15000,
       "reason_code": null, "status": "approved",
       "created_at": "2027-06-12T09:40:00Z", "reviewed_at": "2027-06-12T09:45:00Z",
       "reviewed_by": "fbl@example.org" }}
  ],
  "penalties": [
    {{ "id": "{PENALTY_1}", "event_id": "{EVENT}", "discipline_id": "{DISCIPLINE}",
       "run_id": "{RUN_A1}", "team_id": "{TEAM_A}", "runner_id": null,
       "kind": "sz", "code": "knotengestell.ein_knoten_falsch", "station": "knotengestell",
       "is_disqualification": false, "decider_slot": 0, "delta_ms": 15000,
       "reason_code": null, "status": "approved",
       "created_at": "2027-06-12T09:40:00Z", "reviewed_at": "2027-06-12T09:45:00Z",
       "reviewed_by": "fbl@example.org" }}
  ]
}}"#
    )
}

pub fn sample_payload() -> DisciplinePayload {
    serde_json::from_str(&sample_payload_json()).expect("fixture parses")
}
