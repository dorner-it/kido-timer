//! End-to-end: start the on-prem server on an ephemeral port and drive
//! the judge flows over real HTTP (reqwest), exactly like the phones.

use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::Mutex;
use reqwest::{redirect, Client, StatusCode};
use serde_json::{json, Value};

use super::rate_limit::RateLimiter;
use super::{http, JudgeHost, Shared};
use crate::cloud::fixtures::{id, sample_payload, DISCIPLINE, RUNNER_B1, RUN_B1, RUN_C1};
use crate::cloud::state::{CloudJudgeHost, CloudState, EventSink};
use crate::cloud::types::{CloudEvent, PenaltyStatus, SyncMode};
use crate::local::{persist, LocalStore};

const KNOTEN_SZ: &str = "knotengestell.ein_knoten_falsch";

fn sink() -> (EventSink, Arc<Mutex<Vec<CloudEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let e2 = events.clone();
    (
        Arc::new(move |evs: Vec<CloudEvent>| e2.lock().extend(evs)),
        events,
    )
}

fn loopback() -> Vec<SocketAddr> {
    vec![SocketAddr::from(([127, 0, 0, 1], 0))]
}

fn client() -> Client {
    Client::builder()
        .redirect(redirect::Policy::none())
        .build()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn judge_flow_over_http() {
    let dir = tempfile::tempdir().unwrap();
    let cloud = CloudState::for_tests(
        LocalStore::new(sample_payload()),
        SyncMode::Offline,
        Some(dir.path().to_path_buf()),
    );
    let (sink, events) = sink();
    assert!(cloud.sync_server_with(sink.clone(), &loopback()));
    assert!(
        !cloud.sync_server_with(sink.clone(), &loopback()),
        "idempotent"
    );
    let port = cloud.server_port().expect("running");
    let status = cloud.onprem_status();
    assert!(status.running);
    assert_eq!(status.port, Some(port));
    let token = status.token.clone().expect("token generated");
    assert_eq!(token.len(), 32);
    assert!(status.clients.is_empty(), "loopback is not a phone");

    let base = format!("http://127.0.0.1:{port}");
    let api = format!("{base}/api/decider/sub-1/{token}");
    let http = client();

    // Catalogue (public).
    let cat: Value = http
        .get(format!("{base}/api/catalogue"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cat.as_array().unwrap().len(), 30);

    // State.
    let resp = http.get(format!("{api}/state")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let state: Value = resp.json().await.unwrap();
    assert_eq!(state["disabled"], false);
    assert_eq!(state["current_runs"].as_array().unwrap().len(), 2);
    assert_eq!(state["discipline"]["lane_count"], 2);

    // Wrong token → 401 with the backend's error envelope.
    let resp = http
        .get(format!("{base}/api/decider/sub-1/wrong/state"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "unauthorized");
    // Unknown owner → 404.
    let resp = http
        .get(format!("{base}/api/decider/other/{token}/state"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    // Unknown station in the query → 400.
    let resp = http
        .get(format!("{api}/state?station=wasserbehaelter"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Submit (201), idempotent re-submit (200).
    let cid = "00000000-0000-4000-8000-0000000000c0";
    let body = json!({ "station": "knotengestell", "run_id": RUN_B1, "code": KNOTEN_SZ, "client_id": cid });
    let resp = http
        .post(format!("{api}/penalties"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let p: Value = resp.json().await.unwrap();
    assert_eq!(p["id"], cid);
    assert_eq!(p["kind"], "sz");
    assert_eq!(p["delta_ms"], 15000);
    assert_eq!(p["status"], "pending");
    let resp = http
        .post(format!("{api}/penalties"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Validation errors.
    let resp = http
        .post(format!("{api}/penalties"))
        .json(&json!({ "station": "knotengestell", "run_id": RUN_B1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let e: Value = resp.json().await.unwrap();
    assert_eq!(e["error"]["code"], "validation");
    let resp = http
        .post(format!("{api}/penalties"))
        .json(&json!({ "station": "knotengestell", "run_id": RUN_C1, "code": KNOTEN_SZ }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT, "run not current");
    let resp = http
        .post(format!("{api}/penalties"))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let resp = http
        .post(format!("{api}/penalties"))
        .header("content-type", "application/json")
        .body(vec![b' '; 70 * 1024])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // The judge sees its entry (station filter).
    let state: Value = http
        .get(format!("{api}/state?station=knotengestell"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = state["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&cid));

    // The organizer's list refreshes via the event, and it is persisted.
    assert!(events
        .lock()
        .iter()
        .any(|e| matches!(e, CloudEvent::SnapshotChanged { .. })));
    let saved = persist::load_discipline(dir.path(), id(DISCIPLINE))
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.store.penalty(cid.parse().unwrap()).unwrap().status,
        PenaltyStatus::Pending
    );

    // Withdraw: other station 403, own station 204, again 404.
    let resp = http
        .delete(format!("{api}/penalties/{cid}?station=startlinie"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let resp = http
        .delete(format!("{api}/penalties/{cid}?station=knotengestell"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = http
        .delete(format!("{api}/penalties/{cid}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = http
        .delete(format!("{api}/penalties/not-a-uuid"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Lineup (Startlinie): 200 with the run; non-current run 409; other
    // station 403.
    let resp = http
        .put(format!("{api}/runs/{RUN_B1}/lineup"))
        .json(&json!({ "runner_ids": [RUNNER_B1], "station": "startlinie" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let run: Value = resp.json().await.unwrap();
    assert_eq!(run["lineup"], json!([RUNNER_B1]));
    assert_eq!(run["lineup_confirmed_by"], "startlinie");
    let resp = http
        .put(format!("{api}/runs/{RUN_C1}/lineup"))
        .json(&json!({ "runner_ids": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let resp = http
        .put(format!("{api}/runs/{RUN_B1}/lineup"))
        .json(&json!({ "runner_ids": [], "station": "verteilerlinie" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let lineup_in_store = cloud.with_inner(|inner| inner.view().unwrap().runs[1].lineup.clone());
    assert_eq!(lineup_in_store, vec![id(RUNNER_B1)]);

    // Static judge UI + helper pages.
    let resp = http.get(format!("{base}/judge/")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("window.__KIDO_API_BASE__=\"\""));
    assert!(html.contains("window.__KIDO_OWNER_SUB__=\"sub-1\""));
    let resp = http
        .get(format!("{base}/judge?t={token}&o=sub-1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.headers()["location"],
        format!("/judge/?t={token}&o=sub-1").as_str()
    );
    let resp = http.get(format!("{base}/")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::FOUND);
    let resp = http
        .get(format!("{base}/login?expired=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.text().await.unwrap().contains("QR-Code"));
    let resp = http.get(format!("{base}/api/nope")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let e: Value = resp.json().await.unwrap();
    assert_eq!(e["error"]["code"], "not_found");

    // Token rotation: old phones get 401.
    cloud.with_inner(|inner| inner.rotate_onprem_token().unwrap());
    let resp = http.get(format!("{api}/state")).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let new_token = cloud.onprem_status().token.unwrap();
    assert_ne!(new_token, token);
    let resp = http
        .get(format!("{base}/api/decider/sub-1/{new_token}/state"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Leaving on-prem stops the server.
    cloud.with_inner(|inner| inner.set_mode(SyncMode::Live, false));
    assert!(cloud.sync_server_with(sink.clone(), &loopback()));
    assert!(!cloud.onprem_status().running);
    let mut stopped = false;
    for _ in 0..50 {
        if http
            .get(format!("{base}/api/catalogue"))
            .send()
            .await
            .is_err()
        {
            stopped = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(stopped, "server must stop accepting connections");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rate_limit_answers_429_over_http() {
    let cloud = CloudState::for_tests(LocalStore::new(sample_payload()), SyncMode::Offline, None);
    let (sink, _) = sink();
    // Real host, tight limiter.
    let host: Arc<dyn JudgeHost> = Arc::new(CloudJudgeHost::new(cloud.inner_arc(), sink));
    let shared = Arc::new(Shared::new(host, RateLimiter::new(2.0, 0.0)));
    let app = http::router(shared);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let http = client();
    let url = format!("http://127.0.0.1:{port}/api/catalogue");
    assert_eq!(
        http.get(&url).send().await.unwrap().status(),
        StatusCode::OK
    );
    assert_eq!(
        http.get(&url).send().await.unwrap().status(),
        StatusCode::OK
    );
    let resp = http.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let e: Value = resp.json().await.unwrap();
    assert_eq!(e["error"]["code"], "rate_limited");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_does_not_start_live_or_without_discipline() {
    let cloud = CloudState::for_tests(LocalStore::new(sample_payload()), SyncMode::Live, None);
    let (sink, _) = sink();
    assert!(!cloud.sync_server_with(sink.clone(), &loopback()));
    assert!(!cloud.onprem_status().running);
    let empty = CloudState::default();
    assert!(!empty.sync_server_with(sink, &loopback()));
    assert!(empty.onprem_status().token.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn start_error_is_reported_when_ports_busy() {
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let cloud = CloudState::for_tests(LocalStore::new(sample_payload()), SyncMode::Offline, None);
    let (sink, _) = sink();
    assert!(cloud.sync_server_with(sink, &[busy.local_addr().unwrap()]));
    let status = cloud.onprem_status();
    assert!(!status.running);
    assert!(status.error.unwrap().contains("Kein freier Port"));
}

/// Manual smoke test: serve the embedded judge UI with the sample
/// discipline for `KIDO_SMOKE_SECS` seconds (default 120) on
/// `KIDO_SMOKE_PORT` (default 8787) and print the judge URL.
/// `KIDO_SMOKE_SECS=300 cargo test smoke_serve -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn smoke_serve() {
    let secs: u64 = std::env::var("KIDO_SMOKE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let port: u16 = std::env::var("KIDO_SMOKE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8787);
    let cloud = CloudState::for_tests(LocalStore::new(sample_payload()), SyncMode::Offline, None);
    let (sink, _) = sink();
    cloud.sync_server_with(sink, &[SocketAddr::from(([127, 0, 0, 1], port))]);
    let status = cloud.onprem_status();
    println!(
        "SMOKE http://127.0.0.1:{}/judge/?t={}&o=sub-1",
        status.port.unwrap(),
        status.token.unwrap()
    );
    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
}
