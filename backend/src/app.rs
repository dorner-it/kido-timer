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

/// Stateless router used by the `/api/health` integration test. The binary
/// uses [`router`] instead.
pub fn health_router() -> Router {
    Router::new().route("/api/health", get(health))
}

/// Full router with state. `/api/health` is included for liveness probes
/// even though it doesn't read state.
pub fn router(state: AppState) -> Router {
    Router::<AppState>::new()
        .route("/api/health", get(health))
        .nest("/api/auth", routes::auth::router())
        .with_state(state)
}
