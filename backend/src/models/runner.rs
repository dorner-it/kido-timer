// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Geschlecht des Läufers. **Legacy**: no longer required and no longer
/// written by the UI (the Wertungsgruppe is ticked by the club, there is
/// no gender logic). The field stays readable so existing rows load;
/// new rows carry `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runner {
    pub id: Uuid,
    pub team_id: Uuid,
    pub event_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: NaiveDate,
    /// Position within the team's lineup. Stable across disciplines —
    /// e.g. for relay, position 1 always runs the first leg.
    pub position: u32,
    /// Optional and legacy (see [`Gender`]). Missing on most rows →
    /// `None`.
    #[serde(default)]
    pub gender: Option<Gender>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
