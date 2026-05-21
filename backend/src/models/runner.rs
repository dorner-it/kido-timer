// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
