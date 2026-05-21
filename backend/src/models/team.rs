// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A team belongs to an event, not to a discipline — the same crew may
/// compete in multiple disciplines on the same day. Per-discipline data
/// (start number, lane) lives on `TeamEntry`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub event_id: Uuid,
    pub name: String,
    /// Display order within the event. Used by the roster spreadsheet to
    /// give the user a stable, manually-orderable list independent of any
    /// per-discipline start number.
    pub position: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
