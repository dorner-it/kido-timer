// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Web-app projection of an organizer. Cognito owns identity; we only
/// mirror what's needed for query convenience and display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Cognito `sub` claim. Stable identifier across token refreshes.
    pub sub: String,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
