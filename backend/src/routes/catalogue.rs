// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! `GET /api/catalogue` — the fixed Gruppenstafette error catalogue
//! (public, no auth, static). Source: `kido_core::catalogue::CATALOGUE`.

use axum::Json;
use axum::routing::get;
use serde::Serialize;

use crate::models::{PenaltyKind, StationKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogueItem {
    pub code: &'static str,
    pub station: StationKind,
    pub kind: PenaltyKind,
    pub label_de: &'static str,
}

/// All catalogue entries in sheet order.
pub fn items() -> Vec<CatalogueItem> {
    kido_core::catalogue::CATALOGUE
        .iter()
        .map(|e| CatalogueItem {
            code: e.code,
            station: e.station,
            kind: e.kind,
            label_de: e.label_de,
        })
        .collect()
}

pub async fn list() -> Json<Vec<CatalogueItem>> {
    Json(items())
}

/// Stateless router so it can be mounted (and tested) without
/// `AppState`.
pub fn router<S: Clone + Send + Sync + 'static>() -> axum::Router<S> {
    axum::Router::new().route("/api/catalogue", get(list))
}
