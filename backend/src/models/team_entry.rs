// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::wertungsgruppe::Wertungsgruppe;

/// Meldung — declares that a team competes in a specific discipline,
/// with a discipline-local start number and (for two-lane modes) a lane
/// assignment. A team can have multiple entries across disciplines.
///
/// `wg` is derived from the roster + the event date at create/update
/// time and **persisted**. It deliberately doesn't recompute when the
/// roster changes afterwards — competitions need stable classifications
/// across results processing. Re-stamp by re-`POST`ing the entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamEntry {
    pub team_id: Uuid,
    pub discipline_id: Uuid,
    pub event_id: Uuid,
    pub start_number: u32,
    pub lane: Option<u8>,
    /// Tolerates pre-existing rows (created before WG was introduced):
    /// `#[serde(default)]` → `None`. Newly-written entries always have
    /// it populated unless the route returned a derivation error.
    #[serde(default)]
    pub wg: Option<Wertungsgruppe>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
