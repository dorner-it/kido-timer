// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Route-level checks for `GET /api/events/{id}/disciplines/{discipline_id}/results`
//! without AWS: the state carries SDK clients that are never called
//! (no credentials, no network), so only the routing + auth gate run.
//! The scoring mapping itself is unit-tested in `src/results.rs`.

use std::sync::Arc;

use aws_sdk_dynamodb::config::{BehaviorVersion, Region};
use axum::body::Body;
use axum::http::{Request, StatusCode};
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

async fn get(uri: &str) -> (StatusCode, serde_json::Value) {
    let response = ki_do_timer_backend::app::router(offline_state())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn results_route_requires_organizer_session() {
    let uri = format!(
        "/api/events/{}/disciplines/{}/results",
        Uuid::new_v4(),
        Uuid::new_v4()
    );
    let (status, body) = get(&uri).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");
}

#[tokio::test]
async fn results_route_rejects_non_uuid_path() {
    let (status, _) = get("/api/events/not-a-uuid/disciplines/x/results").await;
    // Rejected either by the auth gate or by the UUID path extractor —
    // never 200, never 500.
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST,
        "got {status}"
    );
}

#[tokio::test]
async fn unknown_sibling_path_is_404() {
    let uri = format!(
        "/api/events/{}/disciplines/{}/resultz",
        Uuid::new_v4(),
        Uuid::new_v4()
    );
    let (status, _) = get(&uri).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
