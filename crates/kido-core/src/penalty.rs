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
    pub fn as_str(self) -> &'static str {
        todo!()
    }

    /// Whether the error counts towards the result: `Pending` and
    /// `Approved` count (pending ones make the result *provisional*),
    /// `Rejected` does not.
    pub fn counts(self) -> bool {
        todo!()
    }
}
