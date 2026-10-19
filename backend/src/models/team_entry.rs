// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::runner::Runner;
use crate::models::wertungsgruppe::{Wertungsgruppe, WgWarning, check_roster};

/// Meldung — declares that a team competes in a specific discipline,
/// with a discipline-local start number, an optional lane and the
/// Wertungsgruppe **ticked by the club** (input, never derived).
///
/// `wg` is `None` only on legacy rows written before 2027 (UI shows
/// "WG fehlt"); every POST requires it. Moving a team to another WG
/// ("Umsetzen") appends to the append-only `wg_history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamEntry {
    pub team_id: Uuid,
    pub discipline_id: Uuid,
    pub event_id: Uuid,
    pub start_number: u32,
    pub lane: Option<u8>,
    #[serde(default)]
    pub wg: Option<Wertungsgruppe>,
    #[serde(default)]
    pub wg_history: Vec<WgHistoryEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One "Umsetzen" step: who moved the team from which WG to which, when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WgHistoryEntry {
    /// `None` when the legacy entry had no WG yet.
    pub from: Option<Wertungsgruppe>,
    pub to: Wertungsgruppe,
    /// Organizer e-mail (or Cognito `sub` when no e-mail is known).
    pub by: String,
    pub at: DateTime<Utc>,
    #[serde(default)]
    pub note: Option<String>,
}

/// API projection of a [`TeamEntry`]: the stored entry plus the
/// age warnings computed on every read (never stored, never exported).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TeamEntryView {
    #[serde(flatten)]
    pub entry: TeamEntry,
    pub wg_warnings: Vec<WgWarning>,
}

impl TeamEntryView {
    /// Attach the age warnings for `roster` (the team's runners) in the
    /// given competition year (the event year; 31.12.-age).
    pub fn new(entry: TeamEntry, roster: &[Runner], competition_year: i32) -> Self {
        let wg_warnings = compute_wg_warnings(&entry, roster, competition_year);
        Self { entry, wg_warnings }
    }
}

/// Age check for one entry. Empty when the entry has no WG (legacy) or
/// the roster is empty. Only runners of the entry's team are considered.
pub fn compute_wg_warnings(
    entry: &TeamEntry,
    roster: &[Runner],
    competition_year: i32,
) -> Vec<WgWarning> {
    let Some(wg) = entry.wg else {
        return Vec::new();
    };
    let runners: Vec<(Uuid, chrono::NaiveDate)> = roster
        .iter()
        .filter(|r| r.team_id == entry.team_id)
        .map(|r| (r.id, r.birth_date))
        .collect();
    if runners.is_empty() {
        return Vec::new();
    }
    check_roster(wg, &runners, competition_year)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_json() -> serde_json::Value {
        serde_json::json!({
            "team_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "start_number": 3,
            "lane": 1,
            "created_at": "2026-05-29T12:00:00Z",
            "updated_at": "2026-05-29T12:00:00Z"
        })
    }

    #[test]
    fn pre_wg_row_reads_with_null_wg_and_empty_history() {
        let e: TeamEntry = serde_json::from_value(legacy_json()).unwrap();
        assert_eq!(e.wg, None);
        assert!(e.wg_history.is_empty());
        let out = serde_json::to_value(&e).unwrap();
        assert_eq!(out["wg"], serde_json::Value::Null);
        assert_eq!(out["wg_history"], serde_json::json!([]));
        assert!(out.get("wg_warnings").is_none(), "never stored/exported");
    }

    #[test]
    fn derived_wg_row_still_loads() {
        let mut j = legacy_json();
        j["wg"] = serde_json::json!("wg2");
        let e: TeamEntry = serde_json::from_value(j).unwrap();
        assert_eq!(e.wg, Some(Wertungsgruppe::Wg2));
    }

    #[test]
    fn history_roundtrips() {
        let mut j = legacy_json();
        j["wg"] = serde_json::json!("wg1");
        j["wg_history"] = serde_json::json!([
            { "from": "wg2", "to": "wg1", "by": "fbl@example.org", "at": "2027-06-12T09:00:00Z", "note": "2 Mädchen" },
            { "from": null, "to": "wg2", "by": "sub-1", "at": "2027-06-11T09:00:00Z" }
        ]);
        let e: TeamEntry = serde_json::from_value(j).unwrap();
        assert_eq!(e.wg_history.len(), 2);
        assert_eq!(e.wg_history[0].from, Some(Wertungsgruppe::Wg2));
        assert_eq!(e.wg_history[0].note.as_deref(), Some("2 Mädchen"));
        assert_eq!(e.wg_history[1].from, None);
        assert_eq!(e.wg_history[1].note, None);
        let out = serde_json::to_value(&e).unwrap();
        let back: TeamEntry = serde_json::from_value(out).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn view_flattens_entry_and_adds_warnings() {
        let e: TeamEntry = serde_json::from_value(legacy_json()).unwrap();
        // No WG → no check → no kido-core call.
        let v = TeamEntryView::new(e, &[], 2027);
        let out = serde_json::to_value(&v).unwrap();
        assert_eq!(out["start_number"], 3);
        assert_eq!(out["wg"], serde_json::Value::Null);
        assert_eq!(out["wg_warnings"], serde_json::json!([]));
    }
}
