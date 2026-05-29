// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::station::StationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PenaltyStatus {
    Pending,
    Approved,
    Rejected,
}

impl PenaltyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PenaltyStatus::Pending => "pending",
            PenaltyStatus::Approved => "approved",
            PenaltyStatus::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Penalty {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub run_id: Uuid,
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,

    /// Originating station. `None` only for rows written before the
    /// stations feature shipped; every penalty created from now on is
    /// expected to carry one.
    #[serde(default)]
    pub station: Option<StationKind>,

    /// True when the decider flagged the run as a disqualification.
    /// DQ penalties carry a sentinel `delta_ms` (long enough to drop
    /// the team to last place) so existing time-based code paths keep
    /// working, but UIs surface them as a dedicated state.
    #[serde(default)]
    pub is_disqualification: bool,

    /// Decider slot 1..=4. **Deprecated** in favour of `station`: the
    /// slot identified a person, the station identifies the post. Kept
    /// on the row so existing penalties remain readable and so the
    /// legacy slot-based QR-code routes (which still know only their
    /// slot) can keep submitting. Defaults to 0 in the new flow when
    /// the legacy slot is unknown.
    pub decider_slot: u8,

    pub delta_ms: i32,
    pub reason_code: Option<String>,
    pub status: PenaltyStatus,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::station::StationKind;

    fn dummy_uuid() -> Uuid {
        Uuid::nil()
    }

    #[test]
    fn legacy_row_without_station_deserialises_with_none() {
        // Mirrors what a Penalty written before the stations feature
        // looks like on the wire. Missing `station` and
        // `is_disqualification` must default rather than fail.
        let legacy_json = serde_json::json!({
            "id": dummy_uuid(),
            "event_id": dummy_uuid(),
            "discipline_id": dummy_uuid(),
            "run_id": dummy_uuid(),
            "team_id": dummy_uuid(),
            "runner_id": null,
            "decider_slot": 2,
            "delta_ms": 5000,
            "reason_code": "kleiner_fehler",
            "status": "pending",
            "created_at": "2026-05-29T12:00:00Z",
            "reviewed_at": null,
            "reviewed_by": null
        });
        let parsed: Penalty = serde_json::from_value(legacy_json).expect("parses");
        assert_eq!(parsed.station, None);
        assert!(!parsed.is_disqualification);
        assert_eq!(parsed.decider_slot, 2);
    }

    #[test]
    fn new_row_with_station_and_dq_roundtrips() {
        let p = Penalty {
            id: dummy_uuid(),
            event_id: dummy_uuid(),
            discipline_id: dummy_uuid(),
            run_id: dummy_uuid(),
            team_id: dummy_uuid(),
            runner_id: None,
            station: Some(StationKind::Knoten),
            is_disqualification: true,
            decider_slot: 0,
            delta_ms: 999_000,
            reason_code: Some("dsq".to_string()),
            status: PenaltyStatus::Pending,
            created_at: chrono::Utc::now(),
            reviewed_at: None,
            reviewed_by: None,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["station"], "knoten");
        assert_eq!(json["is_disqualification"], true);
        let back: Penalty = serde_json::from_value(json).unwrap();
        assert_eq!(back.station, Some(StationKind::Knoten));
        assert!(back.is_disqualification);
    }
}
