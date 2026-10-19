// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Error kinds and review states.

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
}
