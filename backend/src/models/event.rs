// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A "Tagesveranstaltung" — the umbrella for one day's competitions.
/// Holds the shared infrastructure (location, decider tokens, active
/// state). Disciplines hang off this; teams and runners also live at the
/// event level so the same crew can compete in multiple disciplines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "EventWire")]
pub struct Event {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: NaiveDate,
    pub location: Option<String>,
    /// Exactly zero or one event per user is active at a time. The
    /// decider page reads the active event.
    pub is_active: bool,
    /// The discipline currently on the lane. Decider penalties attach to
    /// the runs inside this discipline.
    pub current_discipline_id: Option<Uuid>,
    /// **Legacy** single current run, kept for old clients. Always
    /// equals `current_run_ids.first()`.
    pub current_run_id: Option<Uuid>,
    /// Runs currently being timed — one per active lane, at most the
    /// discipline's `lane_count`.
    pub current_run_ids: Vec<Uuid>,
    /// 4 opaque tokens, one per strafrichter slot. **Deprecated** —
    /// kept so existing QR codes that include `/:slot/:token/` keep
    /// working. New flows use the single `station_token` below.
    pub decider_tokens: [String; 4],
    /// Single 32-byte URL-safe base64 token shared by every strafrichter
    /// at this event. The decider picks their station inside the app
    /// rather than encoding it into the URL. Generated at event
    /// creation; backfilled on first save for legacy events.
    pub station_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Event {
    /// Set the current runs, keeping the legacy `current_run_id` mirror
    /// in sync.
    pub fn set_current_runs(&mut self, run_ids: Vec<Uuid>) {
        self.current_run_id = run_ids.first().copied();
        self.current_run_ids = run_ids;
    }
}

/// Read-time migration for the current-run fields.
///
/// - Rows written before 2027 only have `current_run_id`: read as
///   `current_run_ids = [current_run_id]` (or `[]`).
/// - Rows with `current_run_ids` are authoritative; `current_run_id` is
///   re-derived as its first element so both fields always agree.
pub fn normalize_current_runs(
    legacy: Option<Uuid>,
    ids: Option<Vec<Uuid>>,
) -> (Option<Uuid>, Vec<Uuid>) {
    match ids {
        Some(ids) => (ids.first().copied(), ids),
        None => (legacy, legacy.into_iter().collect()),
    }
}

#[derive(Deserialize)]
struct EventWire {
    id: Uuid,
    owner_sub: String,
    name: String,
    date: NaiveDate,
    location: Option<String>,
    is_active: bool,
    #[serde(default)]
    current_discipline_id: Option<Uuid>,
    #[serde(default)]
    current_run_id: Option<Uuid>,
    #[serde(default)]
    current_run_ids: Option<Vec<Uuid>>,
    decider_tokens: [String; 4],
    #[serde(default)]
    station_token: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<EventWire> for Event {
    fn from(w: EventWire) -> Self {
        let (current_run_id, current_run_ids) =
            normalize_current_runs(w.current_run_id, w.current_run_ids);
        Event {
            id: w.id,
            owner_sub: w.owner_sub,
            name: w.name,
            date: w.date,
            location: w.location,
            is_active: w.is_active,
            current_discipline_id: w.current_discipline_id,
            current_run_id,
            current_run_ids,
            decider_tokens: w.decider_tokens,
            station_token: w.station_token,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_json(current_run_id: Option<Uuid>) -> serde_json::Value {
        serde_json::json!({
            "id": Uuid::nil(),
            "owner_sub": "sub-1",
            "name": "Kreisausscheid",
            "date": "2027-06-12",
            "location": null,
            "is_active": true,
            "current_discipline_id": Uuid::nil(),
            "current_run_id": current_run_id,
            "decider_tokens": ["a", "b", "c", "d"],
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        })
    }

    #[test]
    fn legacy_row_with_current_run_reads_as_single_element_list() {
        let run = Uuid::new_v4();
        let e: Event = serde_json::from_value(legacy_json(Some(run))).unwrap();
        assert_eq!(e.current_run_id, Some(run));
        assert_eq!(e.current_run_ids, vec![run]);
        assert_eq!(e.station_token, "");
    }

    #[test]
    fn legacy_row_without_current_run_reads_as_empty_list() {
        let e: Event = serde_json::from_value(legacy_json(None)).unwrap();
        assert_eq!(e.current_run_id, None);
        assert!(e.current_run_ids.is_empty());
    }

    #[test]
    fn current_run_ids_win_over_stale_legacy_field() {
        let (a, b, stale) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let mut j = legacy_json(Some(stale));
        j["current_run_ids"] = serde_json::json!([a, b]);
        let e: Event = serde_json::from_value(j).unwrap();
        assert_eq!(e.current_run_ids, vec![a, b]);
        assert_eq!(e.current_run_id, Some(a));
    }

    #[test]
    fn empty_current_run_ids_clear_legacy_field() {
        let mut j = legacy_json(Some(Uuid::new_v4()));
        j["current_run_ids"] = serde_json::json!([]);
        let e: Event = serde_json::from_value(j).unwrap();
        assert_eq!(e.current_run_id, None);
        assert!(e.current_run_ids.is_empty());
    }

    #[test]
    fn output_carries_both_fields() {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        let mut e: Event = serde_json::from_value(legacy_json(None)).unwrap();
        e.set_current_runs(vec![a, b]);
        let out = serde_json::to_value(&e).unwrap();
        assert_eq!(out["current_run_id"], serde_json::json!(a));
        assert_eq!(out["current_run_ids"], serde_json::json!([a, b]));
        let back: Event = serde_json::from_value(out).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn normalize_helper() {
        let a = Uuid::new_v4();
        assert_eq!(normalize_current_runs(None, None), (None, vec![]));
        assert_eq!(normalize_current_runs(Some(a), None), (Some(a), vec![a]));
        assert_eq!(
            normalize_current_runs(Some(a), Some(vec![])),
            (None, vec![])
        );
    }
}
