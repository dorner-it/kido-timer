//! HTTP layer of the on-prem server (axum): routing, body limit, per-IP
//! rate limit, JSON (de)serialization with the backend's error envelope.
//! The logic lives in [`super::JudgeHost`] / `local::judge`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{ConnectInfo, DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use super::assets::{self, Asset};
use super::{Shared, BODY_LIMIT};
use crate::local::judge::{
    catalogue_items, ApiError, StationLineupInput, StationQuery, SubmitStationPenaltyInput,
};

type AppState = Arc<Shared>;

pub fn router(shared: AppState) -> Router {
    Router::new()
        .route("/", get(root_redirect))
        .route("/judge", get(judge_redirect))
        .route("/judge/", get(judge_index))
        .route("/judge/{*path}", get(judge_file))
        .route("/login", get(login_page))
        .route("/favicon.svg", get(favicon))
        .route("/api/catalogue", get(catalogue))
        .route("/api/decider/{owner_sub}/{token}/state", get(state))
        .route("/api/decider/{owner_sub}/{token}/penalties", post(submit))
        .route(
            "/api/decider/{owner_sub}/{token}/penalties/{penalty_id}",
            delete(withdraw),
        )
        .route(
            "/api/decider/{owner_sub}/{token}/runs/{run_id}/lineup",
            put(lineup),
        )
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn_with_state(shared.clone(), guard))
        .with_state(shared)
}

// ===== responses ==============================================================

fn json<T: Serialize>(status: StatusCode, value: &T) -> Response {
    match serde_json::to_vec(value) {
        Ok(body) => (
            status,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            body,
        )
            .into_response(),
        Err(e) => error(ApiError::Internal(e.to_string())),
    }
}

pub fn error(e: ApiError) -> Response {
    if let ApiError::Internal(detail) = &e {
        eprintln!("on-prem server: internal error: {detail}");
    }
    let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    json(status, &e.body())
}

fn asset(a: Asset) -> Response {
    let status = StatusCode::from_u16(a.status).unwrap_or(StatusCode::OK);
    let cache = if a.immutable {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        status,
        [
            (header::CONTENT_TYPE, a.content_type),
            (header::CACHE_CONTROL, cache),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        a.body,
    )
        .into_response()
}

fn redirect(location: &str) -> Response {
    let mut resp = StatusCode::FOUND.into_response();
    if let Ok(v) = HeaderValue::from_str(location) {
        resp.headers_mut().insert(header::LOCATION, v);
    }
    resp
}

fn parse_body<T: DeserializeOwned>(body: Result<Bytes, BytesRejection>) -> Result<T, ApiError> {
    let bytes = body.map_err(|e| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::PayloadTooLarge
        } else {
            ApiError::Validation(e.body_text())
        }
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::Validation(format!("ungültiger Body: {e}")))
}

fn parse_uuid(raw: &str, what: &str) -> Result<Uuid, ApiError> {
    raw.parse()
        .map_err(|_| ApiError::Validation(format!("{what} ist keine gültige UUID")))
}

fn parse_query(q: Result<Query<StationQuery>, QueryRejection>) -> Result<StationQuery, ApiError> {
    q.map(|Query(q)| q)
        .map_err(|e| ApiError::Validation(e.body_text()))
}

// ===== middleware =============================================================

/// Per-IP rate limit + "phones seen" bookkeeping.
async fn guard(
    State(shared): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = addr.ip();
    if !shared.allow(ip) {
        return error(ApiError::RateLimited);
    }
    if shared.note_client(ip) {
        shared.host.client_seen(ip);
    }
    next.run(req).await
}

// ===== static =================================================================

async fn root_redirect() -> Response {
    redirect("/judge/")
}

async fn judge_redirect(uri: Uri) -> Response {
    match uri.query() {
        Some(q) => redirect(&format!("/judge/?{q}")),
        None => redirect("/judge/"),
    }
}

async fn judge_index(State(shared): State<AppState>) -> Response {
    asset(assets::judge_asset("", shared.host.owner_sub().as_deref()))
}

async fn judge_file(State(shared): State<AppState>, Path(path): Path<String>) -> Response {
    asset(assets::judge_asset(
        &path,
        shared.host.owner_sub().as_deref(),
    ))
}

async fn favicon(State(shared): State<AppState>) -> Response {
    asset(assets::judge_asset(
        "favicon.svg",
        shared.host.owner_sub().as_deref(),
    ))
}

async fn login_page() -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        assets::LOGIN_PAGE,
    )
        .into_response()
}

async fn not_found(uri: Uri) -> Response {
    if uri.path().starts_with("/api/") {
        error(ApiError::NotFound(format!("route {}", uri.path())))
    } else {
        (StatusCode::NOT_FOUND, "Nicht gefunden").into_response()
    }
}

// ===== API ====================================================================

async fn catalogue() -> Response {
    json(StatusCode::OK, &catalogue_items())
}

async fn state(
    State(shared): State<AppState>,
    Path((owner_sub, token)): Path<(String, String)>,
    query: Result<Query<StationQuery>, QueryRejection>,
) -> Response {
    let result = parse_query(query).and_then(|q| shared.host.state(&owner_sub, &token, q.station));
    match result {
        Ok(s) => json(StatusCode::OK, &s),
        Err(e) => error(e),
    }
}

async fn submit(
    State(shared): State<AppState>,
    Path((owner_sub, token)): Path<(String, String)>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let result = parse_body::<SubmitStationPenaltyInput>(body)
        .and_then(|input| shared.host.submit(&owner_sub, &token, input));
    match result {
        Ok((true, p)) => json(StatusCode::CREATED, &p),
        Ok((false, p)) => json(StatusCode::OK, &p),
        Err(e) => error(e),
    }
}

async fn withdraw(
    State(shared): State<AppState>,
    Path((owner_sub, token, penalty_id)): Path<(String, String, String)>,
    query: Result<Query<StationQuery>, QueryRejection>,
) -> Response {
    let result = parse_uuid(&penalty_id, "penalty_id").and_then(|id| {
        let q = parse_query(query)?;
        shared.host.withdraw(&owner_sub, &token, id, q.station)
    });
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error(e),
    }
}

async fn lineup(
    State(shared): State<AppState>,
    Path((owner_sub, token, run_id)): Path<(String, String, String)>,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let result = parse_uuid(&run_id, "run_id").and_then(|run_id| {
        let input = parse_body::<StationLineupInput>(body)?;
        shared.host.lineup(&owner_sub, &token, run_id, input)
    });
    match result {
        Ok(run) => json(StatusCode::OK, &run),
        Err(e) => error(e),
    }
}
