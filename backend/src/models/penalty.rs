// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::station::StationKind;

pub use kido_core::penalty::{PenaltyKind, PenaltyStatus, SZ_PENALTY_MS};

/// A judge-recorded error on a run.
///
/// 2027 model: `kind` (SZ = +15 s, FV = run invalid) plus the catalogue
/// `code` replace the old free `delta_ms` / `is_disqualification` pair.
/// Both old fields stay on the wire for old clients:
/// - `is_disqualification` is always `kind == fv` on output and ignored
///   on input;
/// - `delta_ms` is `15000` for catalogue SZ, `0` for catalogue FV, and
///   whatever was stored for legacy rows / the legacy slot route.
///
/// Read-time migration: rows without `kind` read as
/// `is_disqualification ? fv : sz`; rows without `code` read as `null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "PenaltyWire")]
pub struct Penalty {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub run_id: Uuid,
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,

    pub kind: PenaltyKind,
    /// Catalogue code (`"<station>.<slug>"`). `None` on legacy rows and
    /// on penalties from the legacy slot route.
    pub code: Option<String>,

    /// Originating station (canonical key on output). `None` only for
    /// rows written before the stations feature and for the legacy slot
    /// route.
    pub station: Option<StationKind>,

    /// Output-only mirror of `kind == Fv` for old clients.
    pub is_disqualification: bool,

    /// Decider slot 1..=4. **Deprecated** in favour of `station`; `0`
    /// in the station flow.
    pub decider_slot: u8,

    pub delta_ms: i32,
    pub reason_code: Option<String>,
    pub status: PenaltyStatus,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
}

/// `delta_ms` written for a catalogue penalty of the given kind.
pub fn catalogue_delta_ms(kind: PenaltyKind) -> i32 {
    match kind {
        PenaltyKind::Sz => SZ_PENALTY_MS as i32,
        PenaltyKind::Fv => 0,
    }
}

/// Read-time migration of the penalty kind: an explicit `kind` wins,
/// otherwise the legacy `is_disqualification` flag decides.
pub fn migrate_kind(kind: Option<PenaltyKind>, is_disqualification: bool) -> PenaltyKind {
    kind.unwrap_or(if is_disqualification {
        PenaltyKind::Fv
    } else {
        PenaltyKind::Sz
    })
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
    delta_ms: i32,
    #[serde(default)]
    reason_code: Option<String>,
    status: PenaltyStatus,
    created_at: DateTime<Utc>,
    #[serde(default)]
    reviewed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    reviewed_by: Option<String>,
}

impl From<PenaltyWire> for Penalty {
    fn from(w: PenaltyWire) -> Self {
        let kind = migrate_kind(w.kind, w.is_disqualification);
        Penalty {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_json() -> serde_json::Value {
        // A Penalty written before the stations feature: no `station`,
        // no `is_disqualification`, no `kind`, no `code`.
        serde_json::json!({
            "id": Uuid::nil(),
            "event_id": Uuid::nil(),
            "discipline_id": Uuid::nil(),
            "run_id": Uuid::nil(),
            "team_id": Uuid::nil(),
            "runner_id": null,
            "decider_slot": 2,
            "delta_ms": 5000,
            "reason_code": "kleiner_fehler",
            "status": "pending",
            "created_at": "2026-05-29T12:00:00Z",
            "reviewed_at": null,
            "reviewed_by": null
        })
    }

    #[test]
    fn pre_stations_row_reads_as_sz_without_code() {
        let p: Penalty = serde_json::from_value(legacy_json()).expect("parses");
        assert_eq!(p.station, None);
        assert_eq!(p.kind, PenaltyKind::Sz);
        assert_eq!(p.code, None);
        assert!(!p.is_disqualification);
        assert_eq!(p.decider_slot, 2);
        // Legacy delta is preserved verbatim.
        assert_eq!(p.delta_ms, 5000);
    }

    #[test]
    fn legacy_dq_row_reads_as_fv_with_canonical_station() {
        let mut j = legacy_json();
        j["station"] = serde_json::json!("knoten");
        j["is_disqualification"] = serde_json::json!(true);
        j["delta_ms"] = serde_json::json!(999_000);
        j["decider_slot"] = serde_json::json!(0);
        let p: Penalty = serde_json::from_value(j).unwrap();
        assert_eq!(p.kind, PenaltyKind::Fv);
        assert!(p.is_disqualification);
        assert_eq!(p.station, Some(StationKind::Knotengestell));
        assert_eq!(p.delta_ms, 999_000);
        let out = serde_json::to_value(&p).unwrap();
        assert_eq!(out["station"], "knotengestell");
        assert_eq!(out["kind"], "fv");
        assert_eq!(out["is_disqualification"], true);
        assert_eq!(out["code"], serde_json::Value::Null);
    }

    #[test]
    fn legacy_kuebelspritze_row_reads_as_angriffslinie() {
        let mut j = legacy_json();
        j["station"] = serde_json::json!("kuebelspritze");
        let p: Penalty = serde_json::from_value(j).unwrap();
        assert_eq!(p.station, Some(StationKind::Angriffslinie));
    }

    #[test]
    fn explicit_kind_wins_and_is_disqualification_is_ignored_on_input() {
        let mut j = legacy_json();
        j["kind"] = serde_json::json!("sz");
        j["is_disqualification"] = serde_json::json!(true);
        let p: Penalty = serde_json::from_value(j).unwrap();
        assert_eq!(p.kind, PenaltyKind::Sz);
        assert!(!p.is_disqualification, "mirror must follow kind");

        let mut j = legacy_json();
        j["kind"] = serde_json::json!("fv");
        j["is_disqualification"] = serde_json::json!(false);
        let p: Penalty = serde_json::from_value(j).unwrap();
        assert!(p.is_disqualification);
    }

    #[test]
    fn new_row_roundtrips() {
        let p = Penalty {
            id: Uuid::new_v4(),
            event_id: Uuid::nil(),
            discipline_id: Uuid::nil(),
            run_id: Uuid::nil(),
            team_id: Uuid::nil(),
            runner_id: None,
            kind: PenaltyKind::Sz,
            code: Some("knotengestell.ein_knoten_falsch".into()),
            station: Some(StationKind::Knotengestell),
            is_disqualification: false,
            decider_slot: 0,
            delta_ms: catalogue_delta_ms(PenaltyKind::Sz),
            reason_code: None,
            status: PenaltyStatus::Pending,
            created_at: "2027-06-12T10:00:00Z".parse().unwrap(),
            reviewed_at: None,
            reviewed_by: None,
        };
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["kind"], "sz");
        assert_eq!(json["code"], "knotengestell.ein_knoten_falsch");
        assert_eq!(json["station"], "knotengestell");
        assert_eq!(json["delta_ms"], 15000);
        assert_eq!(json["is_disqualification"], false);
        assert_eq!(json["status"], "pending");
        let back: Penalty = serde_json::from_value(json).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn catalogue_delta_per_kind() {
        assert_eq!(catalogue_delta_ms(PenaltyKind::Sz), 15_000);
        assert_eq!(catalogue_delta_ms(PenaltyKind::Fv), 0);
    }

    #[test]
    fn migrate_kind_matrix() {
        assert_eq!(migrate_kind(None, false), PenaltyKind::Sz);
        assert_eq!(migrate_kind(None, true), PenaltyKind::Fv);
        assert_eq!(migrate_kind(Some(PenaltyKind::Sz), true), PenaltyKind::Sz);
        assert_eq!(migrate_kind(Some(PenaltyKind::Fv), false), PenaltyKind::Fv);
    }
}
