// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::sync_mode::SyncMode;
use crate::models::timer_mode::{JudgeMode, TimerMode};

/// One "Wettkampf" (discipline) within an event — e.g. Gruppenstafette,
/// Löschangriff. Carries the timer mode, the number of lanes actually
/// used, the judge deployment and the live/offline sync state. The
/// desktop timer app pairs to exactly one discipline at a time.
///
/// Read-time migration: rows written before 2027 carry neither
/// `lane_count` nor `judge_mode`; they read as
/// `mode.default_lane_count()` and `per_station`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "DisciplineWire")]
pub struct Discipline {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub mode: TimerMode,
    /// Lanes in use: Gruppenstaffette 1 | 2 (default 2), Löschangriff 4.
    pub lane_count: u8,
    pub judge_mode: JudgeMode,
    pub sync_mode: SyncMode,
    /// Display order within the event. Lower goes first.
    pub position: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lenient on-the-wire shape: every 2027 field is optional.
#[derive(Deserialize)]
struct DisciplineWire {
    id: Uuid,
    event_id: Uuid,
    name: String,
    mode: TimerMode,
    #[serde(default)]
    lane_count: Option<u8>,
    #[serde(default)]
    judge_mode: Option<JudgeMode>,
    #[serde(default)]
    sync_mode: SyncMode,
    position: u32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DisciplineWire> for Discipline {
    fn from(w: DisciplineWire) -> Self {
        Discipline {
            id: w.id,
            event_id: w.event_id,
            name: w.name,
            mode: w.mode,
            lane_count: w.lane_count.unwrap_or_else(|| w.mode.default_lane_count()),
            judge_mode: w.judge_mode.unwrap_or_default(),
            sync_mode: w.sync_mode,
            position: w.position,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_json(mode: &str) -> serde_json::Value {
        serde_json::json!({
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "name": "Gruppenstafette",
            "mode": mode,
            "position": 1,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        })
    }

    #[test]
    fn new_fields_are_serialized() {
        let d: Discipline = serde_json::from_value(serde_json::json!({
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "name": "GS",
            "mode": "gruppenstaffette",
            "lane_count": 1,
            "judge_mode": "per_station_and_lane",
            "sync_mode": "offline",
            "position": 1,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        }))
        .unwrap();
        assert_eq!(d.lane_count, 1);
        assert_eq!(d.judge_mode, JudgeMode::PerStationAndLane);
        assert_eq!(d.sync_mode, SyncMode::Offline);
        let out = serde_json::to_value(&d).unwrap();
        assert_eq!(out["lane_count"], 1);
        assert_eq!(out["judge_mode"], "per_station_and_lane");
        assert_eq!(out["sync_mode"], "offline");
        let back: Discipline = serde_json::from_value(out).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn legacy_row_defaults_judge_mode_and_sync_mode() {
        // `lane_count` is stated explicitly so this test does not depend
        // on kido-core's `default_lane_count`.
        let mut j = legacy_json("gruppenstaffette");
        j["lane_count"] = serde_json::json!(2);
        let d: Discipline = serde_json::from_value(j).unwrap();
        assert_eq!(d.judge_mode, JudgeMode::PerStation);
        assert_eq!(d.sync_mode, SyncMode::Live);
    }

    #[test]
    fn legacy_row_without_lane_count_uses_mode_default() {
        let d: Discipline = serde_json::from_value(legacy_json("gruppenstaffette")).unwrap();
        assert_eq!(d.lane_count, 2);
        let d: Discipline = serde_json::from_value(legacy_json("loeschangriff")).unwrap();
        assert_eq!(d.lane_count, 4);
    }

    #[test]
    fn legacy_on_prem_alias_reads_as_offline() {
        let mut j = legacy_json("gruppenstaffette");
        j["lane_count"] = serde_json::json!(2);
        j["sync_mode"] = serde_json::json!("on_prem");
        let d: Discipline = serde_json::from_value(j).unwrap();
        assert_eq!(d.sync_mode, SyncMode::Offline);
    }
}
