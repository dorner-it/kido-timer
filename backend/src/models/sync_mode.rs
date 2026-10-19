// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Whether a discipline is run **live** (judges + desktop talk to the
//! cloud API) or **offline** (= on-prem: the desktop runs a local server
//! in the venue network and results are re-uploaded as a `.kido` file).
//!
//! While `Offline`:
//! - Decider input is disabled (the page renders a stub).
//! - Live ingest endpoints reject with 409.
//! - Result-file uploads are accepted.
//!
//! While `Live`:
//! - Result-file uploads are rejected with 409.
//!
//! The type lives in `kido-core`; wire value `on_prem` is accepted as an
//! alias for `offline`.

pub use kido_core::discipline::SyncMode;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_values() {
        assert_eq!(serde_json::to_string(&SyncMode::Live).unwrap(), "\"live\"");
        assert_eq!(
            serde_json::to_string(&SyncMode::Offline).unwrap(),
            "\"offline\""
        );
        assert_eq!(SyncMode::default(), SyncMode::Live);
    }

    #[test]
    fn on_prem_alias_reads_as_offline() {
        let m: SyncMode = serde_json::from_str("\"on_prem\"").unwrap();
        assert_eq!(m, SyncMode::Offline);
    }
}
