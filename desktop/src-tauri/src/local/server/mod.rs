//! On-prem local HTTP server (contract B): the desktop serves the judge UI
//! and the judge API to the phones in the venue LAN.
//!
//! - binds `0.0.0.0:8787` (fallback 8788…8797),
//! - runs on the Tauri async runtime (tokio), stopped gracefully when the
//!   effective mode leaves on-prem, the discipline is closed or the app
//!   exits,
//! - all data access goes through a [`JudgeHost`] (implemented on top of
//!   the persisted local store in `cloud::state`), so the HTTP layer holds
//!   no state besides the per-IP rate limiter and the "phones seen" list.

pub mod assets;
pub mod http;
pub mod net;
pub mod rate_limit;
pub mod token;

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::judge::{ApiResult, StationDeciderState, StationLineupInput, SubmitStationPenaltyInput};
use crate::cloud::types::{Penalty, Run, StationKind};
use rate_limit::RateLimiter;

/// First port tried.
pub const DEFAULT_PORT: u16 = 8787;
/// Last fallback port (the firewall rule covers 8787–8797).
pub const LAST_PORT: u16 = 8797;
/// Max request body (judge requests are tiny).
pub const BODY_LIMIT: usize = 64 * 1024;

/// Data access of the judge API (auth + store), implemented by the cloud
/// state. Every method checks `owner_sub` / `token`.
pub trait JudgeHost: Send + Sync + 'static {
    fn state(
        &self,
        owner_sub: &str,
        token: &str,
        station: Option<StationKind>,
    ) -> ApiResult<StationDeciderState>;
    fn submit(
        &self,
        owner_sub: &str,
        token: &str,
        input: SubmitStationPenaltyInput,
    ) -> ApiResult<(bool, Penalty)>;
    fn withdraw(
        &self,
        owner_sub: &str,
        token: &str,
        penalty_id: Uuid,
        station: Option<StationKind>,
    ) -> ApiResult<()>;
    fn lineup(
        &self,
        owner_sub: &str,
        token: &str,
        run_id: Uuid,
        input: StationLineupInput,
    ) -> ApiResult<Run>;
    /// Owner of the loaded discipline (injected into the judge UI).
    fn owner_sub(&self) -> Option<String>;
    /// A phone (non-loopback IP) talked to the server for the first time.
    fn client_seen(&self, _ip: IpAddr) {}
}

/// A phone that talked to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub ip: String,
    /// Seconds since the last request.
    pub last_seen_secs: u64,
    pub requests: u64,
}

/// Judge link of one LAN address incl. its QR code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeLink {
    pub address: String,
    pub url: String,
    pub qr_svg: String,
}

/// Shown on the on-prem screen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnPremStatus {
    pub running: bool,
    pub port: Option<u16>,
    /// One link per LAN IPv4 address (best first).
    pub links: Vec<JudgeLink>,
    pub token: Option<String>,
    pub owner_sub: Option<String>,
    /// Seconds since the server started.
    pub uptime_secs: Option<u64>,
    pub clients: Vec<ClientInfo>,
    /// Start error (e.g. all ports busy).
    pub error: Option<String>,
    /// The judge UI was embedded at build time.
    pub judge_ui_built: bool,
    /// "Ins Web hochladen" preview: changed runs / penalties to send.
    pub upload_runs: usize,
    pub upload_penalties: usize,
    /// Last successful upload (RFC 3339).
    pub last_upload_at: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Seen {
    last: Instant,
    requests: u64,
}

/// State shared by all request handlers.
pub struct Shared {
    pub host: Arc<dyn JudgeHost>,
    limiter: Mutex<RateLimiter>,
    clients: Mutex<BTreeMap<IpAddr, Seen>>,
}

impl Shared {
    pub fn new(host: Arc<dyn JudgeHost>, limiter: RateLimiter) -> Self {
        Self {
            host,
            limiter: Mutex::new(limiter),
            clients: Mutex::new(BTreeMap::new()),
        }
    }

    /// Rate limit check for `ip` (`false` = 429).
    pub fn allow(&self, ip: IpAddr) -> bool {
        self.limiter.lock().check(ip, Instant::now())
    }

    /// Remember a request of `ip`; loopback (the laptop itself) does not
    /// count as a phone. Returns `true` for a new phone.
    pub fn note_client(&self, ip: IpAddr) -> bool {
        if ip.is_loopback() {
            return false;
        }
        let mut clients = self.clients.lock();
        let now = Instant::now();
        match clients.get_mut(&ip) {
            Some(seen) => {
                seen.last = now;
                seen.requests += 1;
                false
            }
            None => {
                clients.insert(
                    ip,
                    Seen {
                        last: now,
                        requests: 1,
                    },
                );
                true
            }
        }
    }

    pub fn clients(&self) -> Vec<ClientInfo> {
        let now = Instant::now();
        self.clients
            .lock()
            .iter()
            .map(|(ip, seen)| ClientInfo {
                ip: ip.to_string(),
                last_seen_secs: now.saturating_duration_since(seen.last).as_secs(),
                requests: seen.requests,
            })
            .collect()
    }
}

/// Candidate bind addresses: `0.0.0.0:8787` … `0.0.0.0:8797`.
pub fn default_bind_addrs() -> Vec<SocketAddr> {
    (DEFAULT_PORT..=LAST_PORT)
        .map(|p| SocketAddr::from(([0, 0, 0, 0], p)))
        .collect()
}

/// Bind the first free address of `candidates`.
pub fn bind_first(candidates: &[SocketAddr]) -> Result<TcpListener, String> {
    let mut last_err = None;
    for addr in candidates {
        match TcpListener::bind(addr) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(format!("{addr}: {e}")),
        }
    }
    Err(format!(
        "Kein freier Port für den lokalen Server ({})",
        last_err.unwrap_or_else(|| "keine Adresse".into())
    ))
}

/// A running server.
pub struct LocalServer {
    port: u16,
    started: Instant,
    shared: Arc<Shared>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl LocalServer {
    /// Bind (synchronously, so errors surface right away) and serve on the
    /// Tauri async runtime.
    pub fn start(host: Arc<dyn JudgeHost>, candidates: &[SocketAddr]) -> Result<Self, String> {
        let listener = bind_first(candidates)?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Server-Socket: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("Server-Socket: {e}"))?
            .port();
        let shared = Arc::new(Shared::new(host, RateLimiter::default()));
        let app = http::router(shared.clone());
        let (tx, rx) = oneshot::channel::<()>();
        let task = tauri::async_runtime::spawn(async move {
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("on-prem server: {e}");
                    return;
                }
            };
            let service = app.into_make_service_with_connect_info::<SocketAddr>();
            if let Err(e) = axum::serve(listener, service)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await
            {
                eprintln!("on-prem server stopped: {e}");
            }
        });
        Ok(Self {
            port,
            started: Instant::now(),
            shared,
            shutdown: Some(tx),
            task: Some(task),
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn clients(&self) -> Vec<ClientInfo> {
        self.shared.clients()
    }

    /// Graceful shutdown: stop accepting, let in-flight requests finish
    /// (bounded wait).
    pub fn stop(mut self) {
        self.signal_stop();
        if let Some(task) = self.task.take() {
            tauri::async_runtime::spawn(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;
            });
        }
    }

    /// Like [`LocalServer::stop`] but waits (app exit).
    pub fn stop_blocking(mut self) {
        self.signal_stop();
        if let Some(task) = self.task.take() {
            tauri::async_runtime::block_on(async move {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;
            });
        }
    }

    fn signal_stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.signal_stop();
    }
}

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn default_addrs_cover_firewall_range() {
        let addrs = default_bind_addrs();
        assert_eq!(addrs.len(), 11);
        assert_eq!(addrs[0].port(), 8787);
        assert_eq!(addrs[10].port(), 8797);
        assert!(addrs.iter().all(|a| a.ip().is_unspecified()));
    }

    #[test]
    fn bind_falls_back_to_next_free_port() {
        let busy = TcpListener::bind("127.0.0.1:0").unwrap();
        let busy_addr = busy.local_addr().unwrap();
        let free = SocketAddr::from(([127, 0, 0, 1], 0));
        let l = bind_first(&[busy_addr, free]).unwrap();
        assert_ne!(l.local_addr().unwrap().port(), busy_addr.port());
    }

    #[test]
    fn bind_reports_when_all_busy() {
        let busy = TcpListener::bind("127.0.0.1:0").unwrap();
        let err = bind_first(&[busy.local_addr().unwrap()]).unwrap_err();
        assert!(err.contains("Kein freier Port"));
        assert!(bind_first(&[]).is_err());
    }

    struct NoHost;
    impl JudgeHost for NoHost {
        fn state(
            &self,
            _: &str,
            _: &str,
            _: Option<StationKind>,
        ) -> ApiResult<StationDeciderState> {
            Err(super::super::judge::ApiError::Unauthorized)
        }
        fn submit(
            &self,
            _: &str,
            _: &str,
            _: SubmitStationPenaltyInput,
        ) -> ApiResult<(bool, Penalty)> {
            Err(super::super::judge::ApiError::Unauthorized)
        }
        fn withdraw(&self, _: &str, _: &str, _: Uuid, _: Option<StationKind>) -> ApiResult<()> {
            Err(super::super::judge::ApiError::Unauthorized)
        }
        fn lineup(&self, _: &str, _: &str, _: Uuid, _: StationLineupInput) -> ApiResult<Run> {
            Err(super::super::judge::ApiError::Unauthorized)
        }
        fn owner_sub(&self) -> Option<String> {
            None
        }
    }

    #[test]
    fn clients_ignore_loopback_and_count_requests() {
        let shared = Shared::new(Arc::new(NoHost), RateLimiter::default());
        assert!(!shared.note_client(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        let phone = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50));
        assert!(shared.note_client(phone), "first request = new phone");
        assert!(!shared.note_client(phone));
        let c = shared.clients();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].ip, "192.168.1.50");
        assert_eq!(c[0].requests, 2);
    }
}
