// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PenaltyStatus {
    Pending,
    Approved,
    Rejected,
}

impl PenaltyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PenaltyStatus::Pending => "pending",
            PenaltyStatus::Approved => "approved",
            PenaltyStatus::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Penalty {
    pub id: Uuid,
    pub event_id: Uuid,
    pub discipline_id: Uuid,
    pub run_id: Uuid,
    pub team_id: Uuid,
    pub runner_id: Option<Uuid>,
    pub decider_slot: u8,
    pub delta_ms: i32,
    pub reason_code: Option<String>,
    pub status: PenaltyStatus,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
}
