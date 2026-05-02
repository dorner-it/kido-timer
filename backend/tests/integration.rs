// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use ki_do_timer_backend::app::health_router;
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok_true() {
    let response = health_router()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("router responds");

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("body collects")
        .to_bytes();

    let body: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("body is JSON");

    assert_eq!(body, serde_json::json!({ "ok": true }));
}
