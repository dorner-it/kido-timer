// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Meldung — declares that a team competes in a specific discipline,
/// with a discipline-local start number and (for two-lane modes) a lane
/// assignment. A team can have multiple entries across disciplines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamEntry {
    pub team_id: Uuid,
    pub discipline_id: Uuid,
    pub event_id: Uuid,
    pub start_number: u32,
    pub lane: Option<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
