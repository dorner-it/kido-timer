// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Shared domain logic for KI-DO Timer.
//!
//! Used by the web backend (`backend/`) and the desktop app
//! (`desktop/src-tauri`) so that live (cloud) and on-prem (desktop as
//! local server) produce identical results. Pure logic only: no I/O, no
//! async, no storage.
//!
//! Spec: `docs/concept-gruppenstafette-2027.md`.

pub mod catalogue;
pub mod discipline;
pub mod penalty;
pub mod scoring;
pub mod station;
pub mod timing;
pub mod wg;

pub use catalogue::{CATALOGUE, CatalogueEntry};
pub use discipline::{JudgeMode, SyncMode, TimerMode};
pub use penalty::{PenaltyKind, PenaltyStatus, SZ_PENALTY_MS};
pub use station::StationKind;
pub use timing::TimeSource;
pub use wg::Wertungsgruppe;
