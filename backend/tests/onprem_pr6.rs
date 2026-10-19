// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! PR 6 (on-prem) route checks without AWS: the state carries SDK
//! clients that are never called (no credentials, no network), so only
//! routing, method matching and the API-key gate run. The merge and
//! validation rules behind `onprem-upload` are unit-tested in
//! `src/onprem/tests.rs`.

use std::sync::Arc;

use aws_sdk_dynamodb::config::{BehaviorVersion, Region};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use ki_do_timer_backend::auth::cognito::CognitoClient;
use ki_do_timer_backend::auth::jwks::JwksCache;
use ki_do_timer_backend::config::Config;
use ki_do_timer_backend::db::DbClient;
use ki_do_timer_backend::state::AppState;
use tower::ServiceExt;
use uuid::Uuid;

fn offline_state() -> AppState {
    let config = Config {
        env: "test".into(),
        aws_region: "eu-central-1".into(),
        table_name: "ki-do-timer-test".into(),
        cognito_user_pool_id: "eu-central-1_test".into(),
        cognito_client_id: "client".into(),
    };
    let ddb_conf = aws_sdk_dynamodb::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("eu-central-1"))
        .build();
    let idp_conf = aws_sdk_cognitoidentityprovider::Config::builder()
        .behavior_version(aws_sdk_cognitoidentityprovider::config::BehaviorVersion::latest())
        .region(aws_sdk_cognitoidentityprovider::config::Region::new(
            "eu-central-1",
        ))
        .build();
    AppState {
        jwks: Arc::new(JwksCache::new(config.cognito_issuer_url())),
        db: DbClient {
            inner: Arc::new(aws_sdk_dynamodb::Client::from_conf(ddb_conf)),
            table_name: config.table_name.clone(),
        },
        cognito: Arc::new(CognitoClient {
            user_pool_id: config.cognito_user_pool_id.clone(),
            client_id: config.cognito_client_id.clone(),
            client_secret: "secret".into(),
            idp: aws_sdk_cognitoidentityprovider::Client::from_conf(idp_conf),
        }),
        config: Arc::new(config),
    }
}

async fn call(
    method: Method,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = ki_do_timer_backend::app::router(offline_state())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[test]
fn route_table_with_pr6_routes_builds() {
    let _ = ki_do_timer_backend::app::api_router();
}

#[tokio::test]
async fn sync_mode_requires_api_key() {
    let uri = format!("/api/export/disciplines/{}/sync-mode", Uuid::new_v4());
    let (status, body) = call(
        Method::POST,
        &uri,
        serde_json::json!({ "sync_mode": "offline" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn onprem_upload_requires_api_key() {
    let uri = format!("/api/export/disciplines/{}/onprem-upload", Uuid::new_v4());
    let (status, body) = call(
        Method::POST,
        &uri,
        serde_json::json!({ "runs": [], "penalties": [] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn pr6_routes_are_post_only() {
    for suffix in ["sync-mode", "onprem-upload"] {
        let uri = format!("/api/export/disciplines/{}/{suffix}", Uuid::new_v4());
        let (status, _) = call(Method::GET, &uri, serde_json::Value::Null).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{suffix}");
    }
}

#[tokio::test]
async fn non_uuid_discipline_never_succeeds() {
    for suffix in ["sync-mode", "onprem-upload"] {
        let uri = format!("/api/export/disciplines/not-a-uuid/{suffix}");
        let (status, _) = call(Method::POST, &uri, serde_json::json!({})).await;
        assert!(
            status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST,
            "{suffix}: got {status}"
        );
    }
}

#[tokio::test]
async fn unknown_sibling_path_is_404() {
    let uri = format!("/api/export/disciplines/{}/onprem-uploads", Uuid::new_v4());
    let (status, _) = call(Method::POST, &uri, serde_json::json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
