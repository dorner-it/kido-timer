// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Geschlecht des Läufers. Required for new runners (so the
/// `Wertungsgruppe` derivation can dispatch all-male vs all-female
/// rosters), but the field is **optional on the wire** to keep
/// pre-existing items (created before this migration) loadable.
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
    /// Migration: pre-existing runner items in DynamoDB don't carry
    /// `gender`. We tolerate that on read (`#[serde(default)]` → None)
    /// and the routing layer enforces "required" only on POST/PUT.
    #[serde(default)]
    pub gender: Option<Gender>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
