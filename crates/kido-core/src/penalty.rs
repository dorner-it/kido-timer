// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Error kinds and review states.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Fixed time penalty per SZ error: +15 s.
pub const SZ_PENALTY_MS: u32 = 15_000;

/// Kind of a judge-recorded error (Gruppenstafette).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PenaltyKind {
    /// Strafzeit: +15 s, any number per run, run stays valid.
    Sz,
    /// Fehlversuch: run invalid (no time). Must be confirmed by the FBL.
    Fv,
}

/// Review state. Workflow: judge records → `Pending`; FBL confirms
/// (`Approved`) or withdraws (`Rejected`, stays visible crossed out);
/// "Übernehmen" finalizes the run and turns remaining `Pending` into
/// `Approved`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PenaltyStatus {
    Pending,
    Approved,
    Rejected,
}

impl PenaltyStatus {
    /// Wire value: `"pending"`, `"approved"`, `"rejected"`.
    pub fn as_str(self) -> &'static str {
        match self {
            PenaltyStatus::Pending => "pending",
            PenaltyStatus::Approved => "approved",
            PenaltyStatus::Rejected => "rejected",
        }
    }

    /// Whether the error counts towards the result: `Pending` and
    /// `Approved` count (pending ones make the result *provisional*),
    /// `Rejected` does not.
    pub fn counts(self) -> bool {
        matches!(self, PenaltyStatus::Pending | PenaltyStatus::Approved)
    }
}

/// Whether an individual review decision ("Bestätigen" / "Zurücknehmen")
/// can still be undone ("Rückgängig", back to `Pending`).
///
/// Rule: the penalty is decided (`status != Pending`) **and** the run's
/// "Übernehmen" has not frozen it yet, i.e. `penalties_accepted_at` is
/// `None` or the decision was made strictly after it
/// (`reviewed_at > penalties_accepted_at`). "Übernehmen" sets
/// `penalties_accepted_at` to the same instant it writes as `reviewed_at`
/// of the penalties it approves, so those are never undoable. A decided
/// penalty without `reviewed_at` on a run that was taken over counts as
/// frozen (it cannot be shown to be newer).
pub fn can_undo_review(
    status: PenaltyStatus,
    reviewed_at: Option<DateTime<Utc>>,
    penalties_accepted_at: Option<DateTime<Utc>>,
) -> bool {
    if status == PenaltyStatus::Pending {
        return false;
    }
    match penalties_accepted_at {
        None => true,
        Some(accepted) => reviewed_at.is_some_and(|r| r > accepted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sz_penalty_is_15s() {
        assert_eq!(SZ_PENALTY_MS, 15_000);
    }

    #[test]
    fn kind_wire_values() {
        assert_eq!(serde_json::to_string(&PenaltyKind::Sz).unwrap(), "\"sz\"");
        assert_eq!(serde_json::to_string(&PenaltyKind::Fv).unwrap(), "\"fv\"");
        let sz: PenaltyKind = serde_json::from_str("\"sz\"").unwrap();
        let fv: PenaltyKind = serde_json::from_str("\"fv\"").unwrap();
        assert_eq!((sz, fv), (PenaltyKind::Sz, PenaltyKind::Fv));
        assert!(serde_json::from_str::<PenaltyKind>("\"SZ\"").is_err());
        assert!(serde_json::from_str::<PenaltyKind>("\"dq\"").is_err());
    }

    #[test]
    fn status_wire_values_match_as_str() {
        for s in [
            PenaltyStatus::Pending,
            PenaltyStatus::Approved,
            PenaltyStatus::Rejected,
        ] {
            let json = serde_json::to_string(&s).unwrap();
            assert_eq!(json, format!("\"{}\"", s.as_str()));
            let back: PenaltyStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
        assert_eq!(PenaltyStatus::Pending.as_str(), "pending");
        assert_eq!(PenaltyStatus::Approved.as_str(), "approved");
        assert_eq!(PenaltyStatus::Rejected.as_str(), "rejected");
        assert!(serde_json::from_str::<PenaltyStatus>("\"withdrawn\"").is_err());
    }

    #[test]
    fn counts() {
        assert!(PenaltyStatus::Pending.counts());
        assert!(PenaltyStatus::Approved.counts());
        assert!(!PenaltyStatus::Rejected.counts());
    }

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn undo_pending_never() {
        assert!(!can_undo_review(PenaltyStatus::Pending, None, None));
        assert!(!can_undo_review(
            PenaltyStatus::Pending,
            Some(t("2027-06-12T10:05:00Z")),
            Some(t("2027-06-12T10:00:00Z"))
        ));
    }

    #[test]
    fn undo_without_take_over_always_for_decided() {
        for s in [PenaltyStatus::Approved, PenaltyStatus::Rejected] {
            assert!(can_undo_review(s, Some(t("2027-06-12T10:00:00Z")), None));
            assert!(
                can_undo_review(s, None, None),
                "legacy row without reviewed_at"
            );
        }
    }

    #[test]
    fn undo_decided_before_take_over_is_frozen() {
        let accepted = Some(t("2027-06-12T10:00:00Z"));
        for s in [PenaltyStatus::Approved, PenaltyStatus::Rejected] {
            assert!(!can_undo_review(
                s,
                Some(t("2027-06-12T09:59:59Z")),
                accepted
            ));
        }
    }

    #[test]
    fn undo_decided_at_take_over_instant_is_frozen() {
        // Penalties approved by "Übernehmen" itself share the timestamp.
        let at = t("2027-06-12T10:00:00.123Z");
        assert!(!can_undo_review(
            PenaltyStatus::Approved,
            Some(at),
            Some(at)
        ));
        assert!(!can_undo_review(
            PenaltyStatus::Rejected,
            Some(at),
            Some(at)
        ));
    }

    #[test]
    fn undo_decided_after_take_over_is_allowed() {
        let accepted = Some(t("2027-06-12T10:00:00Z"));
        for s in [PenaltyStatus::Approved, PenaltyStatus::Rejected] {
            assert!(can_undo_review(
                s,
                Some(t("2027-06-12T10:00:00.001Z")),
                accepted
            ));
            assert!(can_undo_review(
                s,
                Some(t("2027-06-12T11:00:00Z")),
                accepted
            ));
        }
    }

    #[test]
    fn undo_decided_without_reviewed_at_after_take_over_is_frozen() {
        assert!(!can_undo_review(
            PenaltyStatus::Approved,
            None,
            Some(t("2027-06-12T10:00:00Z"))
        ));
    }
}
