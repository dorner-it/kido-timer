// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::timer_mode::TimerMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Competition {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: NaiveDate,
    pub location: Option<String>,
    pub mode: TimerMode,
    pub is_active: bool,
    pub current_run_id: Option<Uuid>,
    pub decider_tokens: [String; 4],
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
