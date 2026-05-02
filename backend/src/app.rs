// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::{Json, Router, routing::get};
use serde::Serialize;

use crate::routes;
use crate::state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

/// Stateless router used by the `/api/health` integration test.
pub fn health_router() -> Router {
    Router::new().route("/api/health", get(health))
}

/// Composed competitions router: `/api/competitions/...` covers the
/// configurator's CRUD plus its nested teams/runners/runs/penalties paths.
fn competitions_router() -> Router<AppState> {
    routes::competitions::router()
        .merge(routes::teams::router())
        .merge(routes::runners::router())
        .merge(routes::runs::router())
        .merge(routes::penalties::router())
}

pub fn router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/health", get(health))
        .nest("/api/auth", routes::auth::router())
        .nest("/api/competitions", competitions_router())
        .nest("/api/decider", routes::decider::router())
        .with_state(state)
}
