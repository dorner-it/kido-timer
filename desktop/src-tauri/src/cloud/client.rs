use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

use super::types::{
    DisciplineListItem, DisciplinePayload, EventSummary, HmacKeyResponse, MeResponse, Run,
    RunResultRequest,
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

    pub async fn get_discipline(&self, id: Uuid) -> Result<DisciplinePayload, String> {
        self.get_json(&format!("/api/export/disciplines/{id}")).await
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
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read body failed: {e}"))?;

    if status.is_success() {
        return serde_json::from_slice::<T>(&bytes)
            .map_err(|e| format!("parse response failed: {e}"));
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
