// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub competition_id: Uuid,
    pub name: String,
    pub start_number: u32,
}
