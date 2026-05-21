// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::sync_mode::SyncMode;
use crate::models::timer_mode::TimerMode;

/// One "Wettkampf" (discipline) within an event — e.g. Löschangriff,
/// Staffellauf. Carries the timer mode and live/offline sync state. The
/// desktop timer app pairs to exactly one discipline at a time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discipline {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    pub mode: TimerMode,
    #[serde(default)]
    pub sync_mode: SyncMode,
    /// Display order within the event. Lower goes first.
    pub position: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
