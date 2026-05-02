// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runner {
    pub id: Uuid,
    pub team_id: Uuid,
    pub competition_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: NaiveDate,
    pub position: u32,
}
