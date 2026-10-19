// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use kido_core::timing::TimeSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Active => "active",
            RunStatus::Completed => "completed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

/// Who confirmed a run's lineup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineupConfirmedBy {
    /// WR 1 at the Startlinie, from the judge app.
    Startlinie,
    /// The organizer, from the web UI.
    Organizer,
}

/// One row per timing result. Mirrors the desktop app's `RunEntry` (an
/// `originalTimeMs` from the timer plus a `correctionMs` operator-applied
/// adjustment). Runs belong to a discipline; `event_id` is denormalized
/// so a single `Query PK = EVENT#<id>` retrieves all runs of the day.
///
/// 2027 fields (`lineup*`, `time_source`, `hand_time_*`) are all
/// defaulted on read so pre-2027 rows still load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,
    pub run_number: u32,
    pub lane: Option<u8>,
    pub status: RunStatus,
    pub original_time_ms: Option<u32>,
    pub correction_ms: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Runner ids who actually ran (subset of the team roster).
    #[serde(default)]
    pub lineup: Vec<Uuid>,
    #[serde(default)]
    pub lineup_confirmed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub lineup_confirmed_by: Option<LineupConfirmedBy>,
    #[serde(default)]
    pub time_source: TimeSource,
    #[serde(default)]
    pub hand_time_1_ms: Option<u32>,
    #[serde(default)]
    pub hand_time_2_ms: Option<u32>,
    /// Last "Übernehmen" (bulk-approve) of this run's penalties. Review
    /// decisions made up to this instant are frozen; later ones can still
    /// be undone (`kido_core::penalty::can_undo_review`). Additive in v4;
    /// missing → `None`.
    #[serde(default)]
    pub penalties_accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_row_defaults_2027_fields() {
        let j = serde_json::json!({
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "team_id": Uuid::nil(),
            "runner_id": null,
            "run_number": 1,
            "lane": 2,
            "status": "completed",
            "original_time_ms": 47230,
            "correction_ms": 5000,
            "started_at": null,
            "ended_at": null,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        });
        let r: Run = serde_json::from_value(j).unwrap();
        assert!(r.lineup.is_empty());
        assert_eq!(r.lineup_confirmed_at, None);
        assert_eq!(r.lineup_confirmed_by, None);
        assert_eq!(r.time_source, TimeSource::Lichtschranke);
        assert_eq!(r.hand_time_1_ms, None);
        assert_eq!(r.hand_time_2_ms, None);
        assert_eq!(r.penalties_accepted_at, None, "missing field reads as null");
        assert_eq!(r.original_time_ms, Some(47230));
        assert_eq!(r.correction_ms, 5000);

        let out = serde_json::to_value(&r).unwrap();
        assert_eq!(out["lineup"], serde_json::json!([]));
        assert_eq!(out["lineup_confirmed_at"], serde_json::Value::Null);
        assert_eq!(out["lineup_confirmed_by"], serde_json::Value::Null);
        assert_eq!(out["time_source"], "lichtschranke");
        assert_eq!(out["hand_time_1_ms"], serde_json::Value::Null);
        assert_eq!(out["hand_time_2_ms"], serde_json::Value::Null);
        // Always present on output (null until the first "Übernehmen").
        assert_eq!(out["penalties_accepted_at"], serde_json::Value::Null);
    }

    #[test]
    fn new_fields_roundtrip() {
        let runner = Uuid::new_v4();
        let j = serde_json::json!({
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "team_id": Uuid::nil(),
            "runner_id": null,
            "run_number": 1,
            "lane": 1,
            "status": "completed",
            "original_time_ms": 47240,
            "correction_ms": 0,
            "started_at": null,
            "ended_at": null,
            "lineup": [runner],
            "lineup_confirmed_at": "2027-06-12T10:00:00Z",
            "lineup_confirmed_by": "startlinie",
            "time_source": "handstopp",
            "hand_time_1_ms": 47230,
            "hand_time_2_ms": 47245,
            "penalties_accepted_at": "2027-06-12T10:05:00Z",
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        });
        let r: Run = serde_json::from_value(j.clone()).unwrap();
        assert_eq!(r.lineup, vec![runner]);
        assert_eq!(r.lineup_confirmed_by, Some(LineupConfirmedBy::Startlinie));
        assert_eq!(r.time_source, TimeSource::Handstopp);
        assert_eq!(
            r.penalties_accepted_at,
            Some("2027-06-12T10:05:00Z".parse().unwrap())
        );
        assert_eq!(serde_json::to_value(&r).unwrap(), j);
    }

    #[test]
    fn lineup_confirmed_by_wire_values() {
        assert_eq!(
            serde_json::to_value(LineupConfirmedBy::Organizer).unwrap(),
            "organizer"
        );
        assert_eq!(
            serde_json::to_value(LineupConfirmedBy::Startlinie).unwrap(),
            "startlinie"
        );
    }
}
