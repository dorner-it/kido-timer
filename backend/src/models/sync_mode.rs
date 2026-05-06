// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use serde::{Deserialize, Serialize};

/// Whether a competition is being run **live** (organizer pushes timing data
/// from the desktop app via the API) or **offline** (operator runs the event
/// from the standalone app and re-uploads results when finished).
///
/// While `Offline`:
/// - Decider input is disabled (the page renders a stub).
/// - Live ingest endpoints reject with 409.
/// - Result-file uploads are accepted.
///
/// While `Live`:
/// - Result-file uploads are rejected with 409.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    #[default]
    Live,
    Offline,
}
