// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, routing::any, routing::get};
use serde::Serialize;
use serde_json::json;

use crate::routes;
use crate::state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

pub fn health_router() -> Router {
    Router::new().route("/api/health", get(health))
}

/// Composed events router: `/api/events/...` covers organiser CRUD plus
/// nested disciplines/teams/runners/team-entries/runs/penalties.
fn events_router() -> Router<AppState> {
    routes::events::router()
        .merge(routes::disciplines::router())
        .merge(routes::teams::router())
        .merge(routes::runners::router())
        .merge(routes::team_entries::router())
        .merge(routes::runs::router())
        .merge(routes::penalties::router())
}

/// 410 GONE for the v1 `/api/competitions/...` and
/// `/api/export/competitions/...` paths. Both human and machine clients
/// (the desktop timer) need to see a structured error pointing them at
/// v2 so misrouted requests fail loudly rather than silently 404.
async fn v1_gone() -> impl IntoResponse {
    (
        StatusCode::GONE,
        Json(json!({
            "error": {
                "code": "schema_v1_gone",
                "message": "Diese Route wurde durch /api/events/... (schema_version 3) ersetzt. Bitte Client aktualisieren."
            }
        })),
    )
}

pub fn router(state: AppState) -> Router {
    api_router().with_state(state)
}

/// Every route, before the state is attached. Split out so tests can
/// build the full route table (route conflicts panic at build time)
/// without AWS clients.
pub fn api_router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/api/health", get(health))
        .merge(routes::catalogue::router())
        .nest("/api/auth", routes::auth::router())
        .nest("/api/users", routes::users::router())
        .nest("/api/events", events_router())
        .nest("/api/decider", routes::decider::router())
        .nest("/api/export", routes::export::router())
        // v1 gone-handlers. `nest` with a fallback-style any-route makes
        // every request under these prefixes return 410, including with
        // trailing path segments.
        .route("/api/competitions", any(v1_gone))
        .route("/api/competitions/{*rest}", any(v1_gone))
        .route("/api/export/competitions", any(v1_gone))
        .route("/api/export/competitions/{*rest}", any(v1_gone))
}
