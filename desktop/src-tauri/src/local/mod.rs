//! Local result store — the desktop's own copy of the discipline — and
//! the on-prem judge server on top of it.
//!
//! - **Live**: results are posted to the web (`cloud::state`); the store
//!   keeps every change until the web acknowledges it, so failed posts and
//!   crashes never lose a time.
//! - **On-prem** (`SyncMode::Offline`, chosen on the web or overridden on
//!   the desktop): nothing is posted; the store is the master copy,
//!   persisted to disk after every change. The local HTTP server
//!   ([`server`]) serves the judge UI + judge API ([`judge`]) to the
//!   phones in the venue LAN; the organizer reviews in the desktop
//!   ([`organizer`]). Afterwards everything is uploaded to the web in one
//!   request (contract A3) or exported as a signed `.kido`.

pub mod judge;
pub mod organizer;
pub mod persist;
pub mod server;
pub mod store;

pub use persist::PersistedDiscipline;
pub use store::{LocalStore, RunEdit};

use time::macros::format_description;
use time::OffsetDateTime;

/// Current UTC time as RFC 3339 with millisecond precision
/// (`2027-06-12T10:00:00.123Z`) — fixed width, so strings sort
/// chronologically.
pub fn now_millis() -> String {
    OffsetDateTime::now_utc()
        .format(format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
        ))
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_millis_is_fixed_width_rfc3339() {
        let now = now_millis();
        assert_eq!(now.len(), 24, "{now}");
        assert!(now.ends_with('Z'));
        assert_eq!(&now[10..11], "T");
        assert_eq!(&now[19..20], ".");
        assert!(
            time::OffsetDateTime::parse(&now, &time::format_description::well_known::Rfc3339)
                .is_ok()
        );
    }
}
