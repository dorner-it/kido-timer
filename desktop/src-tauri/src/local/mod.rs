//! Local result store — the desktop's own copy of the discipline.
//!
//! This module is the seam between "the desktop times runs" and "where the
//! data goes". Today it backs two flows:
//!
//! - **Live**: results are posted to the web (`cloud::state`); the store
//!   keeps every change until the web acknowledges it, so failed posts and
//!   crashes never lose a time.
//! - **On-prem** (`SyncMode::Offline`, chosen on the web or overridden on
//!   the desktop): nothing is posted; the store is the master copy,
//!   persisted to disk after every change and exported as a signed `.kido`.
//!
//! TODO(on-prem PR 6): the local HTTP server for judges plugs in here. It
//! will read the current runs from [`store::LocalStore::view`] and record
//! penalties / lineups / hand times as further local edits (today only run
//! timing is an edit). Uploading those back to the web (penalties, lineups)
//! is not part of this PR — only run results are uploaded via the existing
//! result endpoint when switching back to live.

pub mod persist;
pub mod store;

pub use persist::PersistedDiscipline;
pub use store::{LocalStore, RunEdit};
