// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Active => "active",
            RunStatus::Completed => "completed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

/// One row per timing result. Mirrors the desktop app's `RunEntry` (an
/// `originalTimeMs` from the timer plus a `correctionMs` operator-applied
/// adjustment). Runs belong to a discipline; `event_id` is denormalized
/// so a single `Query PK = EVENT#<id>` retrieves all runs of the day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,
    pub run_number: u32,
    pub lane: Option<u8>,
    pub status: RunStatus,
    pub original_time_ms: Option<u32>,
    pub correction_ms: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
