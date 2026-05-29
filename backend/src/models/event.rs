// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A "Tagesveranstaltung" — the umbrella for one day's competitions.
/// Holds the shared infrastructure (location, decider tokens, active
/// state). Disciplines hang off this; teams and runners also live at the
/// event level so the same crew can compete in multiple disciplines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub owner_sub: String,
    pub name: String,
    pub date: NaiveDate,
    pub location: Option<String>,
    /// Exactly zero or one event per user is active at a time. The
    /// decider page reads the active event.
    pub is_active: bool,
    /// The discipline currently on the lane. Decider penalties attach to
    /// the run inside this discipline.
    pub current_discipline_id: Option<Uuid>,
    /// Within `current_discipline_id`, the run currently being timed.
    pub current_run_id: Option<Uuid>,
    /// 4 opaque tokens, one per strafrichter slot. **Deprecated** —
    /// kept so existing QR codes that include `/:slot/:token/` keep
    /// working. New flows use the single `station_token` below.
    pub decider_tokens: [String; 4],
    /// Single 32-byte URL-safe base64 token shared by every strafrichter
    /// at this event. The decider picks their station inside the app
    /// rather than encoding it into the URL. Generated at event
    /// creation; backfilled on first save for legacy events.
    #[serde(default)]
    pub station_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
