use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use super::types::{
    Discipline, DisciplineListItem, DisciplinePayload, EventSummary, HmacKeyResponse, MeResponse,
    OnPremUploadRequest, OnPremUploadResponse, Run, RunResultRequest, SyncMode, SyncModeRequest,
};

const USER_AGENT: &str = concat!("KiDo-Timer/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
pub struct CloudClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl CloudClient {
    pub fn new(base_url: String, api_key: String) -> Result<Self, String> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| format!("http client init failed: {e}"))?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn me(&self) -> Result<MeResponse, String> {
        self.get_json("/api/export/me").await
    }

    pub async fn hmac_key(&self) -> Result<HmacKeyResponse, String> {
        self.get_json("/api/export/me/hmac-key").await
    }

    pub async fn list_events(&self) -> Result<Vec<EventSummary>, String> {
        self.get_json("/api/export/events").await
    }

    pub async fn list_disciplines(&self) -> Result<Vec<DisciplineListItem>, String> {
        self.get_json("/api/export/disciplines").await
    }

    /// Live snapshot of one discipline. Only schema v4 is accepted
    /// (checked before the full parse, see [`super::kido::parse_payload`]).
    pub async fn get_discipline(&self, id: Uuid) -> Result<DisciplinePayload, String> {
        let resp = self
            .http
            .get(self.url(&format!("/api/export/disciplines/{id}")))
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        let bytes = success_bytes(resp).await?;
        super::kido::parse_payload(&bytes)
    }

    pub async fn post_run_result(
        &self,
        discipline_id: Uuid,
        run_id: Uuid,
        body: &RunResultRequest,
    ) -> Result<Run, String> {
        let url = self.url(&format!(
            "/api/export/disciplines/{discipline_id}/runs/{run_id}/result"
        ));
        let resp = self
            .http
            .post(url)
            .header("X-API-Key", &self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("post failed: {e}"))?;
        decode(resp).await
    }

    /// A2: switch the web's sync mode of a discipline (e.g. `live` before
    /// result POSTs, which the web rejects with 409 while `offline`).
    pub async fn set_sync_mode(
        &self,
        discipline_id: Uuid,
        mode: SyncMode,
    ) -> Result<Discipline, String> {
        let url = self.url(&format!(
            "/api/export/disciplines/{discipline_id}/sync-mode"
        ));
        let resp = self
            .http
            .post(url)
            .header("X-API-Key", &self.api_key)
            .json(&SyncModeRequest { sync_mode: mode })
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        decode(resp).await
    }

    /// A3: upload the on-prem results (changed runs, all penalties).
    pub async fn onprem_upload(
        &self,
        discipline_id: Uuid,
        body: &OnPremUploadRequest,
    ) -> Result<OnPremUploadResponse, String> {
        let url = self.url(&format!(
            "/api/export/disciplines/{discipline_id}/onprem-upload"
        ));
        let resp = self
            .http
            .post(url)
            .header("X-API-Key", &self.api_key)
            .timeout(std::time::Duration::from_secs(60))
            .json(body)
            .send()
            .await
            .map_err(|e| format!("upload failed: {e}"))?;
        decode(resp).await
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let resp = self
            .http
            .get(self.url(path))
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        decode(resp).await
    }
}

async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, String> {
    let bytes = success_bytes(resp).await?;
    serde_json::from_slice::<T>(&bytes).map_err(|e| format!("parse response failed: {e}"))
}

/// Body of a 2xx response, or the mapped error of any other status.
async fn success_bytes(resp: reqwest::Response) -> Result<Vec<u8>, String> {
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read body failed: {e}"))?;

    if status.is_success() {
        return Ok(bytes.to_vec());
    }

    let parsed: Result<Value, _> = serde_json::from_slice(&bytes);
    let detail = match parsed {
        Ok(v) => v
            .get("error")
            .and_then(|e| {
                let code = e.get("code").and_then(|c| c.as_str()).unwrap_or("");
                let msg = e.get("message").and_then(|m| m.as_str()).unwrap_or("");
                if code.is_empty() && msg.is_empty() {
                    None
                } else {
                    Some(format!("{code}: {msg}"))
                }
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).to_string()),
        Err(_) => String::from_utf8_lossy(&bytes).to_string(),
    };

    Err(match status {
        StatusCode::UNAUTHORIZED => format!("unauthorized: {detail}"),
        StatusCode::FORBIDDEN => format!("forbidden: {detail}"),
        StatusCode::NOT_FOUND => format!("not_found: {detail}"),
        StatusCode::CONFLICT => format!("conflict: {detail}"),
        StatusCode::GONE => format!(
            "gone (alte API entfernt — Web-App und Desktop-App müssen aktuell sein): {detail}"
        ),
        s => format!("http {}: {detail}", s.as_u16()),
    })
}

#[cfg(test)]
mod tests {
    //! A2 / A3 against a mock web API (axum on an ephemeral port).
    use std::sync::Arc;

    use axum::extract::{Path, State};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use axum::{Json, Router};
    use parking_lot::Mutex;
    use serde_json::{json, Value};

    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, DISCIPLINE};
    use crate::cloud::types::OnPremUploadRun;

    #[derive(Default)]
    struct Seen {
        path_id: Option<String>,
        api_key: Option<String>,
        body: Option<Value>,
    }

    type Shared = Arc<Mutex<Seen>>;

    fn remember(seen: &Shared, id: String, headers: &HeaderMap, body: Value) {
        let mut s = seen.lock();
        s.path_id = Some(id);
        s.api_key = headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        s.body = Some(body);
    }

    async fn sync_mode(
        State(seen): State<Shared>,
        Path(id): Path<String>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        remember(&seen, id, &headers, body.clone());
        let mut d = serde_json::to_value(sample_payload().discipline).unwrap();
        d["sync_mode"] = body["sync_mode"].clone();
        (StatusCode::OK, Json(d))
    }

    async fn upload(
        State(seen): State<Shared>,
        Path(id): Path<String>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        remember(&seen, id, &headers, body.clone());
        if body["penalties"].as_array().is_none() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": {"code": "validation", "message": "validation: penalties"}})),
            );
        }
        let runs = body["runs"].as_array().map_or(0, Vec::len);
        let pens = body["penalties"].as_array().map_or(0, Vec::len);
        (
            StatusCode::OK,
            Json(json!({"runs_updated": runs, "penalties_upserted": pens})),
        )
    }

    async fn conflict() -> (StatusCode, Json<Value>) {
        (
            StatusCode::CONFLICT,
            Json(json!({"error": {"code": "conflict", "message": "conflict: nope"}})),
        )
    }

    async fn mock() -> (String, Shared) {
        let seen: Shared = Arc::default();
        let app = Router::new()
            .route("/api/export/disciplines/{id}/sync-mode", post(sync_mode))
            .route("/api/export/disciplines/{id}/onprem-upload", post(upload))
            .route(
                "/api/export/disciplines/{id}/runs/{run}/result",
                post(conflict),
            )
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}/"), seen)
    }

    #[tokio::test]
    async fn a2_switches_web_sync_mode() {
        let (base, seen) = mock().await;
        let client = CloudClient::new(base, "key-1".into()).unwrap();
        let d = client
            .set_sync_mode(id(DISCIPLINE), SyncMode::Live)
            .await
            .unwrap();
        assert_eq!(d.sync_mode, SyncMode::Live);
        let s = seen.lock();
        assert_eq!(s.path_id.as_deref(), Some(DISCIPLINE));
        assert_eq!(s.api_key.as_deref(), Some("key-1"));
        assert_eq!(s.body, Some(json!({"sync_mode": "live"})));
    }

    #[tokio::test]
    async fn a3_uploads_runs_and_penalties() {
        let (base, seen) = mock().await;
        let client = CloudClient::new(base, "key-1".into()).unwrap();
        let p = sample_payload();
        let req = OnPremUploadRequest {
            runs: vec![OnPremUploadRun::from_run(&p.runs[0])],
            penalties: p.penalties.clone(),
            current_run_ids: vec![],
        };
        let resp = client.onprem_upload(id(DISCIPLINE), &req).await.unwrap();
        assert_eq!(resp.runs_updated, 1);
        assert_eq!(resp.penalties_upserted, 1);
        let s = seen.lock();
        assert_eq!(s.api_key.as_deref(), Some("key-1"));
        let body = s.body.clone().unwrap();
        assert_eq!(body, serde_json::to_value(&req).unwrap());
        assert_eq!(body["current_run_ids"], json!([]));
    }

    #[tokio::test]
    async fn error_envelope_is_mapped() {
        let (base, _) = mock().await;
        let client = CloudClient::new(base, "k".into()).unwrap();
        let err = client
            .post_run_result(
                id(DISCIPLINE),
                id(DISCIPLINE),
                &RunResultRequest {
                    run_number: 1,
                    status: crate::cloud::types::RunStatus::Completed,
                    original_time_ms: Some(1),
                    started_at: None,
                    ended_at: None,
                },
            )
            .await
            .unwrap_err();
        assert!(err.starts_with("conflict:"), "{err}");
        assert!(err.contains("conflict: nope"));
    }

    #[tokio::test]
    async fn unreachable_web_is_an_error() {
        let client = CloudClient::new("http://127.0.0.1:9".into(), "k".into()).unwrap();
        let err = client
            .set_sync_mode(id(DISCIPLINE), SyncMode::Live)
            .await
            .unwrap_err();
        assert!(err.contains("request failed"));
    }
}
