//! Cloud session: pairing, discipline snapshot, live result posting,
//! polling, on-prem server + upload — on top of the persisted local result
//! store (`crate::local`).
//!
//! The state machine lives in [`CloudInner`] (pure, unit-tested; every
//! mutation returns the events to emit). [`CloudState`] adds the async
//! HTTP calls, the polling task, the on-prem judge server and the Tauri
//! event bridge.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;
use uuid::Uuid;

use super::client::CloudClient;
use super::keychain;
use super::kido;
use super::types::{
    CloudEvent, CloudIdentity, DisciplineListItem, DisciplinePayload, DivergenceState,
    EventSummary, FailedPost, KidoConflict, KidoEnvelope, OnPremUploadRequest,
    OnPremUploadResponse, OpenKidoResult, PairedAccount, Penalty, Run, RunResultRequest, RunStatus,
    StationKind, SyncMode,
};
use crate::lanes;
use crate::local::judge::{
    self, ApiError, ApiResult, StationDeciderState, StationLineupInput, SubmitStationPenaltyInput,
};
use crate::local::server::{self, net, token, JudgeHost, JudgeLink, LocalServer, OnPremStatus};
use crate::local::{now_millis, persist, LocalStore, PersistedDiscipline, RunEdit};

pub const CLOUD_EVENT: &str = "kido://cloud-event";
const POLL_FAST_MS: u64 = 1500;
const POLL_SLOW_MS: u64 = 5000;

/// Result of recording a lane timing change.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordOutcome {
    /// Run the change was recorded for (`None`: no run on that lane).
    pub run_id: Option<Uuid>,
    /// Whether a live POST was attempted.
    pub posted: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct FailureRecord {
    discipline_id: Uuid,
    body: RunResultRequest,
    error: String,
    failed_at: String,
}

impl FailureRecord {
    fn to_failed_post(&self, run_id: Uuid) -> FailedPost {
        FailedPost {
            run_id,
            discipline_id: self.discipline_id,
            run_number: self.body.run_number,
            status: self.body.status,
            original_time_ms: self.body.original_time_ms,
            started_at: self.body.started_at.clone(),
            ended_at: self.body.ended_at.clone(),
            error: self.error.clone(),
            failed_at: self.failed_at.clone(),
        }
    }

    fn from_failed_post(f: &FailedPost) -> (Uuid, Self) {
        (
            f.run_id,
            Self {
                discipline_id: f.discipline_id,
                body: RunResultRequest {
                    run_number: f.run_number,
                    status: f.status,
                    original_time_ms: f.original_time_ms,
                    started_at: f.started_at.clone(),
                    ended_at: f.ended_at.clone(),
                },
                error: f.error.clone(),
                failed_at: f.failed_at.clone(),
            },
        )
    }
}

/// A planned live POST (returned by [`CloudInner::record_lane`]).
#[derive(Debug, Clone, PartialEq)]
struct PostPlan {
    discipline_id: Uuid,
    run_id: Uuid,
    body: RunResultRequest,
}

/// `(run_id, post plan, events)` of [`CloudInner::record_lane`].
type RecordResult = (Option<Uuid>, Option<PostPlan>, Vec<CloudEvent>);

#[derive(Default)]
pub(crate) struct CloudInner {
    account: Option<PairedAccount>,
    /// Local copy of the selected discipline (`None` = nothing loaded).
    store: Option<LocalStore>,
    /// Sync mode in effect (web default with the desktop override applied;
    /// set by the UI via `cloud_set_sync_mode`).
    effective_mode: SyncMode,
    /// Last body successfully posted per run (dedupe).
    post_memos: HashMap<Uuid, RunResultRequest>,
    failed_posts: HashMap<Uuid, FailureRecord>,
    /// Local changes that differ from the web after going live. While
    /// non-empty, nothing is posted until the operator decides.
    divergence: DivergenceState,
    /// The next remote snapshot must run the divergence check.
    needs_divergence_check: bool,
    /// `<app_data>/local-store`; `None` = no persistence (tests, init error).
    storage_dir: Option<PathBuf>,
    /// Problem while restoring on start, shown once by the UI.
    startup_warning: Option<String>,
}

/// Snapshot equality for change detection. Ignores `exported_at`, which
/// the backend stamps on every request; everything else (runs, lineups,
/// penalties, WGs, current runs, …) counts.
pub fn payload_equiv(a: &DisciplinePayload, b: &DisciplinePayload) -> bool {
    a.schema_version == b.schema_version
        && a.owner_sub == b.owner_sub
        && a.event == b.event
        && a.discipline == b.discipline
        && a.teams == b.teams
        && a.runners == b.runners
        && a.team_entries == b.team_entries
        && a.runs == b.runs
        && a.approved_penalties == b.approved_penalties
        && a.penalties == b.penalties
}

fn now_rfc3339() -> String {
    kido::now_rfc3339().unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

impl CloudInner {
    pub(crate) fn view(&self) -> Option<DisciplinePayload> {
        self.store.as_ref().map(|s| s.view())
    }

    fn store_id(&self) -> Option<Uuid> {
        self.store.as_ref().map(|s| s.discipline_id())
    }

    fn is_live(&self) -> bool {
        self.effective_mode == SyncMode::Live
    }

    /// Live POSTs allowed right now.
    fn may_post(&self) -> bool {
        self.is_live()
            && self.account.is_some()
            && !self.needs_divergence_check
            && self.divergence.is_empty()
    }

    fn failures(&self) -> Vec<FailedPost> {
        let mut out: Vec<FailedPost> = self
            .failed_posts
            .iter()
            .map(|(id, rec)| rec.to_failed_post(*id))
            .collect();
        out.sort_by_key(|f| (f.run_number, f.run_id));
        out
    }

    fn snapshot_event(&self) -> Option<CloudEvent> {
        self.view()
            .map(|snapshot| CloudEvent::SnapshotChanged { snapshot })
    }

    /// Write the store to disk. Returns an error event on failure.
    fn persist(&self) -> Option<CloudEvent> {
        let (dir, store) = (self.storage_dir.as_ref()?, self.store.as_ref()?);
        let data = PersistedDiscipline {
            format: persist::FORMAT_VERSION,
            saved_at: now_rfc3339(),
            effective_mode: self.effective_mode,
            store: store.clone(),
            failed_posts: self.failures(),
        };
        persist::save_discipline(dir, &data)
            .and_then(|_| persist::set_current(dir, Some(store.discipline_id())))
            .err()
            .map(|e| CloudEvent::SnapshotError {
                message: format!("Lokale Sicherung fehlgeschlagen: {e}"),
            })
    }

    /// Saved state of `id`: the in-memory store if it is the loaded
    /// discipline, else the file on disk. A broken file is moved aside
    /// (never overwritten) and reported.
    fn local_state_for(&mut self, id: Uuid) -> (Option<PersistedDiscipline>, Vec<CloudEvent>) {
        if self.store_id() == Some(id) {
            let data = PersistedDiscipline {
                format: persist::FORMAT_VERSION,
                saved_at: now_rfc3339(),
                effective_mode: self.effective_mode,
                store: self.store.clone().expect("checked"),
                failed_posts: self.failures(),
            };
            return (Some(data), Vec::new());
        }
        let Some(dir) = self.storage_dir.clone() else {
            return (None, Vec::new());
        };
        match persist::load_discipline(&dir, id) {
            Ok(data) => (data, Vec::new()),
            Err(e) => {
                let moved = persist::quarantine_discipline(&dir, id)
                    .map(|p| format!(" (gesichert als {})", p.display()))
                    .unwrap_or_default();
                (
                    None,
                    vec![CloudEvent::SnapshotError {
                        message: format!("Lokale Sicherung nicht lesbar{moved}: {e}"),
                    }],
                )
            }
        }
    }

    /// Make `store` the loaded discipline.
    fn install(
        &mut self,
        store: LocalStore,
        failed: &[FailedPost],
        mode: SyncMode,
    ) -> Vec<CloudEvent> {
        let mut store = store;
        if mode == SyncMode::Live {
            // The current-run choice is desk-local on-prem; live, the web
            // organizer sets the current runs.
            store.current_run_ids = None;
        }
        self.needs_divergence_check = mode == SyncMode::Live && store.has_edits();
        self.store = Some(store);
        self.failed_posts = failed.iter().map(FailureRecord::from_failed_post).collect();
        self.post_memos.clear();
        self.effective_mode = mode;
        self.divergence = DivergenceState::default();
        let mut events: Vec<CloudEvent> = self.snapshot_event().into_iter().collect();
        events.push(CloudEvent::FailedPostsChanged {
            failures: self.failures(),
        });
        events.push(divergence_event(&self.divergence));
        events.extend(self.persist());
        events
    }

    /// A newer snapshot of the loaded discipline arrived (poll, picker,
    /// `.kido`). Local edits stay on top; in live mode a pending
    /// divergence check runs against it.
    fn apply_remote(&mut self, remote: DisciplinePayload) -> Vec<CloudEvent> {
        if self.store_id() != Some(remote.discipline.id) {
            return Vec::new();
        }
        let check = self.is_live() && (self.needs_divergence_check || !self.divergence.is_empty());
        let failures_before = self.failures();
        let mut events = Vec::new();

        let store = self.store.as_mut().expect("checked");
        let before = store.view();
        let base_changed = !payload_equiv(&store.base, &remote);
        if base_changed {
            store.adopt_remote(remote);
        }
        let mut new_divergence = None;
        if check {
            let base = store.base.clone();
            store.drop_edits_matching(&base);
            new_divergence = Some(DivergenceState {
                divergences: store.divergence(&base),
                unsynced: store.unsynced_summary(),
            });
        }
        let after = store.view();
        let edits_changed = before.runs != after.runs;
        let edit_ids: Vec<Uuid> = store.run_edits.keys().copied().collect();

        if check {
            self.needs_divergence_check = false;
        }
        if let Some(div) = new_divergence {
            if div != self.divergence {
                self.divergence = div;
                events.push(divergence_event(&self.divergence));
            }
        }
        // A failed post whose edit is gone (web has it) is obsolete.
        self.failed_posts.retain(|id, _| edit_ids.contains(id));
        let failures_after = self.failures();
        if failures_after != failures_before {
            events.push(CloudEvent::FailedPostsChanged {
                failures: failures_after,
            });
        }
        if !payload_equiv(&before, &after) {
            events.push(CloudEvent::SnapshotChanged { snapshot: after });
        }
        if base_changed || edits_changed {
            events.extend(self.persist());
        }
        events
    }

    /// Record a timing change on desktop lane `lane`. Always applied to the
    /// local store (and persisted); returns a POST plan when live.
    fn record_lane(
        &mut self,
        lane: u8,
        overrides: &HashMap<u8, Uuid>,
        status: RunStatus,
        original_time_ms: Option<u32>,
        started_at: Option<String>,
        ended_at: Option<String>,
    ) -> Result<RecordResult, String> {
        let may_post = self.may_post();
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
        let before = store.view();
        let Some(run) = lanes::resolve_run_for_lane(&before, lane, overrides) else {
            return Ok((None, None, Vec::new()));
        };
        let run_id = run.id;
        let body = RunResultRequest {
            run_number: run.run_number,
            status,
            original_time_ms,
            started_at,
            ended_at,
        };
        store.record(run_id, RunEdit::from_request(&body, now_rfc3339()))?;
        let has_edit = store.run_edits.contains_key(&run_id);
        let discipline_id = store.discipline_id();
        let after = store.view();

        let mut events = Vec::new();
        if !payload_equiv(&before, &after) {
            events.push(CloudEvent::SnapshotChanged { snapshot: after });
        }
        events.extend(self.persist());

        let already_posted = self.post_memos.get(&run_id) == Some(&body);
        let plan = (may_post && has_edit && !already_posted).then_some(PostPlan {
            discipline_id,
            run_id,
            body,
        });
        Ok((Some(run_id), plan, events))
    }

    fn post_succeeded(
        &mut self,
        discipline_id: Uuid,
        run_id: Uuid,
        body: &RunResultRequest,
        server_run: super::types::Run,
    ) -> Vec<CloudEvent> {
        let status = server_run.status;
        let mut events = Vec::new();
        self.post_memos.insert(run_id, body.clone());
        let removed = self.failed_posts.remove(&run_id).is_some();
        if self.store_id() == Some(discipline_id) {
            if let Some(store) = self.store.as_mut() {
                store.acknowledge(body, server_run);
            }
            events.extend(self.snapshot_event());
            events.extend(self.persist());
        }
        events.push(CloudEvent::ResultPosted { run_id, status });
        if removed {
            events.push(CloudEvent::FailedPostsChanged {
                failures: self.failures(),
            });
        }
        events
    }

    fn post_failed(
        &mut self,
        discipline_id: Uuid,
        run_id: Uuid,
        body: RunResultRequest,
        error: String,
    ) -> Vec<CloudEvent> {
        let mut events = vec![CloudEvent::ResultPostFailed {
            run_id,
            message: error.clone(),
        }];
        if self.store_id() == Some(discipline_id) {
            self.failed_posts.insert(
                run_id,
                FailureRecord {
                    discipline_id,
                    body,
                    error,
                    failed_at: now_rfc3339(),
                },
            );
            events.push(CloudEvent::FailedPostsChanged {
                failures: self.failures(),
            });
            events.extend(self.persist());
        }
        events
    }

    /// "Verwerfen" on a failed post: drop the post *and* the local edit
    /// (the run shows the web's state again).
    fn dismiss_failed(&mut self, run_id: Uuid) -> Vec<CloudEvent> {
        if self.failed_posts.remove(&run_id).is_none() {
            return Vec::new();
        }
        self.post_memos.remove(&run_id);
        if let Some(store) = self.store.as_mut() {
            store.run_edits.remove(&run_id);
        }
        let mut events = vec![CloudEvent::FailedPostsChanged {
            failures: self.failures(),
        }];
        events.extend(self.snapshot_event());
        events.extend(self.persist());
        events
    }

    /// Apply the effective sync mode. Returns `(start_polling,
    /// stop_polling, events)`.
    pub(crate) fn set_mode(
        &mut self,
        mode: SyncMode,
        polling: bool,
    ) -> (bool, bool, Vec<CloudEvent>) {
        let prev = self.effective_mode;
        self.effective_mode = mode;
        let Some(store) = self.store.as_ref() else {
            return (false, polling, Vec::new());
        };
        let mut events = Vec::new();
        match mode {
            SyncMode::Offline => {
                self.needs_divergence_check = false;
                if !self.divergence.is_empty() {
                    self.divergence = DivergenceState::default();
                    events.push(divergence_event(&self.divergence));
                }
                if prev != mode {
                    events.extend(self.persist());
                }
                (false, polling, events)
            }
            SyncMode::Live => {
                if polling && prev == SyncMode::Live {
                    return (false, false, events);
                }
                self.needs_divergence_check = store.has_edits();
                let dropped_current = self
                    .store
                    .as_mut()
                    .and_then(|s| s.current_run_ids.take())
                    .is_some();
                if dropped_current {
                    events.extend(self.snapshot_event());
                }
                if prev != mode || dropped_current {
                    events.extend(self.persist());
                }
                (self.account.is_some(), false, events)
            }
        }
    }

    /// Payload of "Ins Web hochladen" (A3) for the loaded discipline.
    fn upload_payload(&self) -> Option<(Uuid, OnPremUploadRequest)> {
        self.store
            .as_ref()
            .map(|s| (s.discipline_id(), s.upload_payload()))
    }

    /// A3 succeeded: fold the uploaded state into the base, drop what it
    /// covered, unblock live posting.
    fn upload_succeeded(
        &mut self,
        discipline_id: Uuid,
        uploaded: &OnPremUploadRequest,
    ) -> Vec<CloudEvent> {
        if self.store_id() != Some(discipline_id) {
            return Vec::new();
        }
        let failures_before = self.failures();
        let store = self.store.as_mut().expect("checked");
        store.mark_synced(uploaded, now_millis());
        let edit_ids: Vec<Uuid> = store.run_edits.keys().copied().collect();
        self.failed_posts.retain(|id, _| edit_ids.contains(id));
        self.post_memos.clear();
        self.needs_divergence_check = false;
        self.divergence = DivergenceState::default();
        let mut events = vec![divergence_event(&self.divergence)];
        let failures_after = self.failures();
        if failures_after != failures_before {
            events.push(CloudEvent::FailedPostsChanged {
                failures: failures_after,
            });
        }
        events.extend(self.snapshot_event());
        events.extend(self.persist());
        events
    }

    /// "Lokale Änderungen verwerfen": the web's state wins.
    fn discard_local(&mut self) -> Vec<CloudEvent> {
        if let Some(store) = self.store.as_mut() {
            store.discard_edits();
        }
        self.failed_posts.clear();
        self.post_memos.clear();
        self.divergence = DivergenceState::default();
        self.needs_divergence_check = false;
        let mut events = vec![
            divergence_event(&self.divergence),
            CloudEvent::FailedPostsChanged {
                failures: Vec::new(),
            },
        ];
        events.extend(self.snapshot_event());
        events.extend(self.persist());
        events
    }

    /// Unload the discipline. Its file stays on disk.
    fn unload(&mut self) -> Vec<CloudEvent> {
        let mut events: Vec<CloudEvent> = self.persist().into_iter().collect();
        if let Some(dir) = &self.storage_dir {
            if let Err(e) = persist::set_current(dir, None) {
                events.push(CloudEvent::SnapshotError { message: e });
            }
        }
        self.store = None;
        self.failed_posts.clear();
        self.post_memos.clear();
        self.divergence = DivergenceState::default();
        self.needs_divergence_check = false;
        events.push(CloudEvent::FailedPostsChanged {
            failures: Vec::new(),
        });
        events.push(divergence_event(&self.divergence));
        events
    }

    /// Restore the discipline that was loaded when the app last ran.
    fn restore(&mut self, dir: PathBuf) {
        match persist::restore_current(&dir) {
            Ok(Some(data)) => {
                self.failed_posts = data
                    .failed_posts
                    .iter()
                    .map(FailureRecord::from_failed_post)
                    .collect();
                self.effective_mode = data.effective_mode;
                self.store = Some(data.store);
            }
            Ok(None) => {}
            Err(e) => {
                let moved = persist::load_current(&dir)
                    .and_then(|id| persist::quarantine_discipline(&dir, id).ok())
                    .map(|p| format!(" (gesichert als {})", p.display()))
                    .unwrap_or_default();
                let _ = persist::set_current(&dir, None);
                self.startup_warning = Some(format!(
                    "Lokale Sicherung konnte nicht geladen werden{moved}: {e}"
                ));
            }
        }
        self.storage_dir = Some(dir);
    }

    // ── on-prem ─────────────────────────────────────────────────────

    fn is_on_prem(&self) -> bool {
        self.store.is_some() && self.effective_mode == SyncMode::Offline
    }

    pub(crate) fn onprem_token(&self) -> Option<String> {
        self.store.as_ref().and_then(|s| s.onprem_token.clone())
    }

    /// Access code of the loaded discipline, generated (and persisted) on
    /// first use.
    fn ensure_onprem_token(&mut self) -> Result<(String, Vec<CloudEvent>), String> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
        if let Some(t) = store
            .onprem_token
            .clone()
            .filter(|t| token::is_valid_token(t))
        {
            return Ok((t, Vec::new()));
        }
        let t = token::generate_token()?;
        store.onprem_token = Some(t.clone());
        Ok((t, self.persist().into_iter().collect()))
    }

    /// "Neuer Zugangscode": phones with the old code get 401.
    pub(crate) fn rotate_onprem_token(&mut self) -> Result<(String, Vec<CloudEvent>), String> {
        let t = token::generate_token()?;
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
        store.onprem_token = Some(t.clone());
        Ok((t, self.persist().into_iter().collect()))
    }

    /// Judge read access (owner + token checked).
    pub(crate) fn judge_read<T>(
        &self,
        owner_sub: &str,
        token: &str,
        f: impl FnOnce(&DisciplinePayload) -> ApiResult<T>,
    ) -> ApiResult<T> {
        let store = self.authorize_judge(owner_sub, token)?;
        f(&store.view())
    }

    fn authorize_judge(&self, owner_sub: &str, token: &str) -> ApiResult<&LocalStore> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| ApiError::NotFound("no active event".into()))?;
        judge::authorize(
            Some(&store.base),
            store.onprem_token.as_deref(),
            owner_sub,
            token,
        )?;
        if self.effective_mode != SyncMode::Offline {
            return Err(ApiError::Conflict(
                "Wettkampf läuft live; bitte die Web-App verwenden".into(),
            ));
        }
        Ok(store)
    }

    /// Judge write (owner + token checked): applies `f` to the store,
    /// persists and returns the events (snapshot changed).
    pub(crate) fn judge_write<T>(
        &mut self,
        owner_sub: &str,
        token: &str,
        f: impl FnOnce(&mut LocalStore, &str) -> ApiResult<T>,
    ) -> (ApiResult<T>, Vec<CloudEvent>) {
        if let Err(e) = self.authorize_judge(owner_sub, token) {
            return (Err(e), Vec::new());
        }
        let store = self.store.as_mut().expect("authorized");
        let before = store.view();
        let result = f(store, &now_millis());
        let after = store.view();
        let mut events = Vec::new();
        if result.is_ok() && !payload_equiv(&before, &after) {
            events.push(CloudEvent::SnapshotChanged { snapshot: after });
            if let Some(ev) = self.persist() {
                // The judge's change is applied in memory; the operator
                // sees the persistence problem.
                events.push(ev);
            }
        }
        (result, events)
    }

    /// Organizer action (desktop UI, on-prem only).
    pub(crate) fn organizer<T>(
        &mut self,
        f: impl FnOnce(&mut LocalStore, &str, &str) -> Result<T, String>,
    ) -> Result<(T, Vec<CloudEvent>), String> {
        if !self.is_on_prem() {
            return Err(if self.store.is_none() {
                "Kein Wettkampf geladen".into()
            } else {
                "Nur im On-prem-Betrieb — im Live-Betrieb wird im Web geprüft".into()
            });
        }
        let by = self
            .account
            .as_ref()
            .map(|a| a.email.clone())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "Desktop (On-prem)".to_string());
        let store = self.store.as_mut().expect("checked");
        let before = store.view();
        let value = f(store, &by, &now_millis())?;
        let after = store.view();
        let mut events = Vec::new();
        if !payload_equiv(&before, &after) {
            events.push(CloudEvent::SnapshotChanged { snapshot: after });
            events.extend(self.persist());
        }
        Ok((value, events))
    }
}

#[derive(Default)]
pub struct CloudState {
    inner: Arc<Mutex<CloudInner>>,
    poll_task: Mutex<Option<JoinHandle<()>>>,
    /// On-prem judge server (running only on-prem with a discipline).
    server: Mutex<Option<LocalServer>>,
    /// Last server start error (shown on the on-prem screen).
    server_error: Mutex<Option<String>>,
}

/// Judge API host on top of the shared cloud state (used by the on-prem
/// server). Writes are persisted and emitted as `snapshotChanged`, so the
/// organizer's lists refresh when a phone submits.
pub(crate) struct CloudJudgeHost {
    inner: Arc<Mutex<CloudInner>>,
    emit: EventSink,
}

/// Event sink (Tauri in the app, a collector in tests).
pub(crate) type EventSink = Arc<dyn Fn(Vec<CloudEvent>) + Send + Sync>;

impl CloudJudgeHost {
    pub(crate) fn new(inner: Arc<Mutex<CloudInner>>, emit: EventSink) -> Self {
        Self { inner, emit }
    }

    fn write<T>(
        &self,
        owner_sub: &str,
        token: &str,
        f: impl FnOnce(&mut LocalStore, &str) -> ApiResult<T>,
    ) -> ApiResult<T> {
        let (result, events) = self.inner.lock().judge_write(owner_sub, token, f);
        if !events.is_empty() {
            (self.emit)(events);
        }
        result
    }
}

impl JudgeHost for CloudJudgeHost {
    fn state(
        &self,
        owner_sub: &str,
        token: &str,
        station: Option<StationKind>,
    ) -> ApiResult<StationDeciderState> {
        self.inner
            .lock()
            .judge_read(owner_sub, token, |view| judge::station_state(view, station))
    }

    fn submit(
        &self,
        owner_sub: &str,
        token: &str,
        input: SubmitStationPenaltyInput,
    ) -> ApiResult<(bool, Penalty)> {
        self.write(owner_sub, token, |store, now| {
            judge::submit_penalty(store, input, Uuid::new_v4(), now)
        })
    }

    fn withdraw(
        &self,
        owner_sub: &str,
        token: &str,
        penalty_id: Uuid,
        station: Option<StationKind>,
    ) -> ApiResult<()> {
        self.write(owner_sub, token, |store, now| {
            judge::withdraw_penalty(store, penalty_id, station, now)
        })
    }

    fn lineup(
        &self,
        owner_sub: &str,
        token: &str,
        run_id: Uuid,
        input: StationLineupInput,
    ) -> ApiResult<Run> {
        self.write(owner_sub, token, |store, now| {
            judge::set_lineup(store, run_id, input, now)
        })
    }

    fn owner_sub(&self) -> Option<String> {
        self.inner
            .lock()
            .store
            .as_ref()
            .map(|s| s.base.owner_sub.clone())
    }

    fn client_seen(&self, ip: IpAddr) {
        (self.emit)(vec![CloudEvent::OnPremClientSeen { ip: ip.to_string() }]);
    }
}

/// Result of "Ins Web hochladen".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadOutcome {
    pub runs_updated: u32,
    pub penalties_upserted: u32,
    pub runs_sent: usize,
    pub penalties_sent: usize,
}

impl CloudState {
    /// Test helper: a cloud state with `store` loaded in `mode`
    /// (persistence under `dir` when given).
    #[cfg(test)]
    pub(crate) fn for_tests(store: LocalStore, mode: SyncMode, dir: Option<PathBuf>) -> Self {
        let mut inner = CloudInner::default();
        if let Some(dir) = dir {
            inner.restore(dir);
        }
        inner.install(store, &[], mode);
        Self {
            inner: Arc::new(Mutex::new(inner)),
            ..Default::default()
        }
    }

    /// Test helper: the shared inner state.
    #[cfg(test)]
    pub(crate) fn inner_arc(&self) -> Arc<Mutex<CloudInner>> {
        self.inner.clone()
    }

    /// Test helper: run `f` on the inner state.
    #[cfg(test)]
    pub(crate) fn with_inner<T>(&self, f: impl FnOnce(&mut CloudInner) -> T) -> T {
        f(&mut self.inner.lock())
    }

    /// Enable persistence under `dir` and restore the last discipline.
    pub fn init_storage(&self, dir: PathBuf) {
        self.inner.lock().restore(dir);
    }

    pub fn take_startup_warning(&self) -> Option<String> {
        self.inner.lock().startup_warning.take()
    }

    pub fn load_from_keychain(&self) -> Result<Option<CloudIdentity>, String> {
        let account = keychain::load()?;
        let identity = account.as_ref().map(|a| a.identity());
        self.inner.lock().account = account;
        Ok(identity)
    }

    pub fn current_identity(&self) -> Option<CloudIdentity> {
        self.inner.lock().account.as_ref().map(|a| a.identity())
    }

    pub fn current_snapshot(&self) -> Option<DisciplinePayload> {
        self.inner.lock().view()
    }

    /// Penalties whose decision comes from the web snapshot (not
    /// undoable on the desktop).
    pub fn web_decided_penalty_ids(&self) -> Vec<Uuid> {
        self.inner
            .lock()
            .store
            .as_ref()
            .map(|s| s.web_decided_penalty_ids())
            .unwrap_or_default()
    }

    pub fn divergences(&self) -> DivergenceState {
        self.inner.lock().divergence.clone()
    }

    fn client(&self) -> Result<CloudClient, String> {
        make_client(&self.inner).ok_or_else(|| "Nicht mit einem Online-Konto gekoppelt".to_string())
    }

    pub async fn pair(
        &self,
        app: AppHandle,
        base_url: String,
        api_key: String,
    ) -> Result<CloudIdentity, String> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let client = CloudClient::new(base_url.clone(), api_key.clone())?;
        let me = client.me().await?;
        let hmac = client.hmac_key().await?;

        let account = PairedAccount {
            base_url,
            api_key,
            hmac_key_b64: hmac.hmac_key,
            sub: me.sub,
            email: me.email,
            display_name: me.display_name,
            key_id: me.key_id,
        };
        keychain::save(&account)?;

        let identity = account.identity();
        {
            let mut guard = self.inner.lock();
            guard.account = Some(account);
            guard.post_memos.clear();
        }
        emit(
            &app,
            CloudEvent::Paired {
                identity: identity.clone(),
            },
        );
        Ok(identity)
    }

    /// Unpair. The loaded discipline and its local results stay.
    pub fn clear(&self, app: AppHandle) -> Result<(), String> {
        self.stop_polling();
        keychain::clear()?;
        let snapshot = {
            let mut guard = self.inner.lock();
            guard.account = None;
            guard.post_memos.clear();
            guard.snapshot_event()
        };
        emit(&app, CloudEvent::Cleared);
        if let Some(ev) = snapshot {
            emit(&app, ev);
        }
        Ok(())
    }

    pub async fn list_events(&self) -> Result<Vec<EventSummary>, String> {
        self.client()?.list_events().await
    }

    pub async fn list_disciplines(&self) -> Result<Vec<DisciplineListItem>, String> {
        self.client()?.list_disciplines().await
    }

    /// Load a discipline from the web. Local results recorded earlier for
    /// it are kept on top; if the web is unreachable the local copy is
    /// used. `mode` = effective sync mode (web default if `None`).
    pub async fn select_discipline(
        &self,
        app: AppHandle,
        id: Uuid,
        mode: Option<SyncMode>,
    ) -> Result<DisciplinePayload, String> {
        self.stop_polling();
        let client = self.client()?;
        let (local, mut events) = self.inner.lock().local_state_for(id);
        let remote = client.get_discipline(id).await;

        let (view, live) = {
            let mut guard = self.inner.lock();
            match (remote, local) {
                (Ok(remote), local) => {
                    let mode = mode.unwrap_or(remote.discipline.sync_mode);
                    let (store, failed) = match local {
                        Some(d) => (d.store, d.failed_posts),
                        None => (LocalStore::new(remote.clone()), Vec::new()),
                    };
                    events.extend(guard.install(store, &failed, mode));
                    events.extend(guard.apply_remote(remote));
                    (guard.view(), mode == SyncMode::Live)
                }
                (Err(e), Some(d)) => {
                    let mode = mode.unwrap_or(d.store.base.discipline.sync_mode);
                    events.extend(guard.install(d.store, &d.failed_posts, mode));
                    events.push(CloudEvent::SnapshotError {
                        message: format!("Web nicht erreichbar — lokale Sicherung geladen ({e})"),
                    });
                    (guard.view(), mode == SyncMode::Live)
                }
                (Err(e), None) => return Err(e),
            }
        };
        emit_all(&app, events);
        self.sync_server(&app);
        if live {
            self.start_polling(app, id);
        }
        view.ok_or_else(|| "Kein Wettkampf geladen".to_string())
    }

    /// Apply the effective sync mode (web default + desktop override) for
    /// the loaded discipline: on-prem stops all network traffic, live
    /// starts polling (with a divergence check if local results exist).
    ///
    /// Going live while the web still says `offline` first switches the web
    /// to live (A2), so result POSTs are not rejected with 409.
    pub async fn set_sync_mode(&self, app: AppHandle, mode: SyncMode) {
        if mode == SyncMode::Live {
            self.ensure_web_live(&app).await;
        }
        self.apply_sync_mode(&app, mode);
    }

    /// A2 `live` when the loaded discipline is `offline` on the web.
    async fn ensure_web_live(&self, app: &AppHandle) {
        let target = {
            let guard = self.inner.lock();
            guard
                .store
                .as_ref()
                .filter(|s| s.base.discipline.sync_mode == SyncMode::Offline)
                .map(|s| s.discipline_id())
        };
        let (Some(id), Ok(client)) = (target, self.client()) else {
            return;
        };
        match client.set_sync_mode(id, SyncMode::Live).await {
            Ok(discipline) => {
                let events = {
                    let mut guard = self.inner.lock();
                    match guard.store.as_mut() {
                        Some(store) if store.discipline_id() == discipline.id => {
                            store.base.discipline = discipline;
                            let mut events: Vec<CloudEvent> =
                                guard.snapshot_event().into_iter().collect();
                            events.extend(guard.persist());
                            events
                        }
                        _ => Vec::new(),
                    }
                };
                emit_all(app, events);
            }
            Err(e) => emit(
                app,
                CloudEvent::SnapshotError {
                    message: format!(
                        "Web konnte nicht auf Live umgestellt werden ({e}) — Ergebnisse bleiben lokal, bis das Web erreichbar ist"
                    ),
                },
            ),
        }
    }

    fn apply_sync_mode(&self, app: &AppHandle, mode: SyncMode) {
        let app = app.clone();
        let polling = self.is_polling();
        let (start, stop, events, id) = {
            let mut guard = self.inner.lock();
            let (start, stop, events) = guard.set_mode(mode, polling);
            (start, stop, events, guard.store_id())
        };
        if stop {
            self.stop_polling();
        }
        emit_all(&app, events);
        self.sync_server(&app);
        if let (true, Some(id)) = (start, id) {
            self.start_polling(app, id);
        }
    }

    pub fn deselect(&self, app: AppHandle) {
        self.stop_polling();
        let events = self.inner.lock().unload();
        emit_all(&app, events);
        self.sync_server(&app);
    }

    // ── on-prem server ───────────────────────────────────────────────

    fn emitter(app: &AppHandle) -> EventSink {
        let app = app.clone();
        Arc::new(move |events: Vec<CloudEvent>| emit_all(&app, events))
    }

    /// Start / stop the judge server to match the state: running iff a
    /// discipline is loaded and the effective mode is on-prem.
    pub fn sync_server(&self, app: &AppHandle) {
        let changed = self.sync_server_with(Self::emitter(app), &server::default_bind_addrs());
        if changed {
            emit(
                app,
                CloudEvent::OnPremChanged {
                    status: self.onprem_status(),
                },
            );
        }
    }

    /// [`CloudState::sync_server`] without Tauri. Returns whether the
    /// server state changed.
    pub(crate) fn sync_server_with(&self, emitter: EventSink, addrs: &[SocketAddr]) -> bool {
        let want = self.inner.lock().is_on_prem();
        let mut server = self.server.lock();
        match (want, server.is_some()) {
            (true, false) => {
                let events = match self.inner.lock().ensure_onprem_token() {
                    Ok((_, events)) => events,
                    Err(e) => {
                        *self.server_error.lock() = Some(e);
                        return true;
                    }
                };
                emitter(events);
                let host = Arc::new(CloudJudgeHost::new(self.inner.clone(), emitter));
                match LocalServer::start(host, addrs) {
                    Ok(s) => {
                        *server = Some(s);
                        *self.server_error.lock() = None;
                    }
                    Err(e) => *self.server_error.lock() = Some(e),
                }
                true
            }
            (false, true) => {
                if let Some(s) = server.take() {
                    s.stop();
                }
                *self.server_error.lock() = None;
                true
            }
            _ => false,
        }
    }

    /// Stop the server and wait briefly (app exit).
    pub fn shutdown_server(&self) {
        if let Some(s) = self.server.lock().take() {
            s.stop_blocking();
        }
    }

    #[cfg(test)]
    pub fn server_port(&self) -> Option<u16> {
        self.server.lock().as_ref().map(|s| s.port())
    }

    /// Status for the on-prem screen (addresses, QR codes, phones).
    pub fn onprem_status(&self) -> OnPremStatus {
        let (token, owner_sub, upload, last_upload_at) = {
            let guard = self.inner.lock();
            (
                guard.onprem_token(),
                guard.store.as_ref().map(|s| s.base.owner_sub.clone()),
                guard
                    .store
                    .as_ref()
                    .map(|s| s.upload_payload())
                    .map(|p| (p.runs.len(), p.penalties.len()))
                    .unwrap_or_default(),
                guard.store.as_ref().and_then(|s| s.last_upload_at.clone()),
            )
        };
        let server = self.server.lock();
        let mut status = OnPremStatus {
            running: server.is_some(),
            port: server.as_ref().map(|s| s.port()),
            token: token.clone(),
            owner_sub: owner_sub.clone(),
            uptime_secs: server.as_ref().map(|s| s.uptime_secs()),
            clients: server.as_ref().map(|s| s.clients()).unwrap_or_default(),
            error: self.server_error.lock().clone(),
            judge_ui_built: server::assets::JUDGE_UI_BUILT,
            links: Vec::new(),
            upload_runs: upload.0,
            upload_penalties: upload.1,
            last_upload_at,
        };
        if let (Some(port), Some(token), Some(owner)) = (status.port, token, owner_sub) {
            status.links = net::list_lan_ipv4()
                .into_iter()
                .map(|ip| {
                    let url = net::judge_url(&ip.to_string(), port, &token, &owner);
                    JudgeLink {
                        address: ip.to_string(),
                        qr_svg: net::qr_svg(&url).unwrap_or_default(),
                        url,
                    }
                })
                .collect();
        }
        status
    }

    /// "Neuer Zugangscode".
    pub fn rotate_onprem_token(&self, app: &AppHandle) -> Result<OnPremStatus, String> {
        let (_, events) = self.inner.lock().rotate_onprem_token()?;
        emit_all(app, events);
        let status = self.onprem_status();
        emit(
            app,
            CloudEvent::OnPremChanged {
                status: status.clone(),
            },
        );
        Ok(status)
    }

    /// Organizer action on the local store (on-prem only), persisted and
    /// emitted.
    pub fn organizer<T>(
        &self,
        app: &AppHandle,
        f: impl FnOnce(&mut LocalStore, &str, &str) -> Result<T, String>,
    ) -> Result<T, String> {
        let (value, events) = self.inner.lock().organizer(f)?;
        emit_all(app, events);
        Ok(value)
    }

    /// "Ins Web hochladen" (A3): refresh the snapshot from the web (keeps
    /// local changes on top), upload changed runs + all penalties, then
    /// mark the store synced. Works in any web sync mode.
    pub async fn upload_onprem(&self, app: AppHandle) -> Result<UploadOutcome, String> {
        let client = self.client()?;
        let id = self
            .inner
            .lock()
            .store_id()
            .ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
        let remote = client.get_discipline(id).await?;
        let (payload, events) = {
            let mut guard = self.inner.lock();
            let events = guard.apply_remote(remote);
            (guard.upload_payload(), events)
        };
        emit_all(&app, events);
        let (discipline_id, payload) =
            payload.ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
        let resp: OnPremUploadResponse = client.onprem_upload(discipline_id, &payload).await?;
        let events = self.inner.lock().upload_succeeded(discipline_id, &payload);
        emit_all(&app, events);
        Ok(UploadOutcome {
            runs_updated: resp.runs_updated,
            penalties_upserted: resp.penalties_upserted,
            runs_sent: payload.runs.len(),
            penalties_sent: payload.penalties.len(),
        })
    }

    fn is_polling(&self) -> bool {
        self.poll_task
            .lock()
            .as_ref()
            .is_some_and(|h| !h.inner().is_finished())
    }

    fn start_polling(&self, app: AppHandle, id: Uuid) {
        self.stop_polling();
        let inner = self.inner.clone();
        let handle = tauri::async_runtime::spawn(async move {
            loop {
                {
                    let guard = inner.lock();
                    if guard.store_id() != Some(id) || !guard.is_live() {
                        break;
                    }
                }
                let Some(client) = make_client(&inner) else {
                    break;
                };
                let cadence = match client.get_discipline(id).await {
                    Ok(remote) => {
                        let (events, fast) = {
                            let mut guard = inner.lock();
                            let events = guard.apply_remote(remote);
                            let fast = guard.view().as_ref().is_some_and(has_active_run);
                            (events, fast)
                        };
                        emit_all(&app, events);
                        if fast {
                            POLL_FAST_MS
                        } else {
                            POLL_SLOW_MS
                        }
                    }
                    Err(e) => {
                        emit(&app, CloudEvent::SnapshotError { message: e });
                        POLL_SLOW_MS
                    }
                };
                sleep(Duration::from_millis(cadence)).await;
            }
        });
        *self.poll_task.lock() = Some(handle);
    }

    fn stop_polling(&self) {
        if let Some(handle) = self.poll_task.lock().take() {
            handle.abort();
        }
    }

    /// Record a timing change for desktop lane `lane` (resolved to a run
    /// via current runs → overrides → active run). Always stored locally
    /// and persisted; posted to the web when live.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_lane_status(
        &self,
        app: AppHandle,
        lane: u8,
        overrides: HashMap<u8, Uuid>,
        status: RunStatus,
        original_time_ms: Option<u32>,
        started_at: Option<String>,
        ended_at: Option<String>,
    ) -> Result<RecordOutcome, String> {
        let (run_id, plan, events) = self.inner.lock().record_lane(
            lane,
            &overrides,
            status,
            original_time_ms,
            started_at,
            ended_at,
        )?;
        emit_all(&app, events);
        let posted = plan.is_some();
        if let Some(plan) = plan {
            // Failure is recorded in the failed-post queue and emitted.
            let _ = self
                .do_post(app, plan.discipline_id, plan.run_id, plan.body)
                .await;
        }
        Ok(RecordOutcome { run_id, posted })
    }

    pub async fn retry_post(&self, app: AppHandle, run_id: Uuid) -> Result<(), String> {
        let (discipline_id, body) = {
            let guard = self.inner.lock();
            let failure = guard
                .failed_posts
                .get(&run_id)
                .ok_or_else(|| format!("no pending failure for run {run_id}"))?;
            (failure.discipline_id, failure.body.clone())
        };
        self.do_post(app, discipline_id, run_id, body).await
    }

    async fn do_post(
        &self,
        app: AppHandle,
        discipline_id: Uuid,
        run_id: Uuid,
        body: RunResultRequest,
    ) -> Result<(), String> {
        let result = match self.client() {
            Ok(client) => client.post_run_result(discipline_id, run_id, &body).await,
            Err(e) => Err(e),
        };
        match result {
            Ok(updated) => {
                let events =
                    self.inner
                        .lock()
                        .post_succeeded(discipline_id, run_id, &body, updated);
                emit_all(&app, events);
                Ok(())
            }
            Err(e) => {
                let events = self
                    .inner
                    .lock()
                    .post_failed(discipline_id, run_id, body, e.clone());
                emit_all(&app, events);
                Err(e)
            }
        }
    }

    pub fn clear_failed_post(&self, app: AppHandle, run_id: Uuid) {
        let events = self.inner.lock().dismiss_failed(run_id);
        emit_all(&app, events);
    }

    pub fn list_failed_posts(&self) -> Vec<FailedPost> {
        self.inner.lock().failures()
    }

    /// Resolve a divergence: upload all local changes in one on-prem
    /// upload (A3: runs, penalties, lineups, hand times) or discard them.
    pub async fn resolve_divergence(&self, app: AppHandle, upload: bool) -> Result<(), String> {
        if !upload {
            let events = self.inner.lock().discard_local();
            emit_all(&app, events);
            return Ok(());
        }
        self.upload_onprem(app).await.map(|_| ())
    }

    pub fn open_kido_file(
        &self,
        app: AppHandle,
        path: String,
        force: bool,
    ) -> Result<OpenKidoResult, String> {
        let bytes = std::fs::read(&path).map_err(|e| format!("read failed: {e}"))?;
        let envelope: KidoEnvelope =
            serde_json::from_slice(&bytes).map_err(|e| format!("envelope parse failed: {e}"))?;

        let (hmac_key_b64, expected_sub) = {
            let guard = self.inner.lock();
            let account = guard.account.as_ref().ok_or_else(|| {
                "Nicht gekoppelt — erst die Desktop-App mit dem Web-Konto koppeln".to_string()
            })?;
            (account.hmac_key_b64.clone(), account.sub.clone())
        };

        let payload = kido::verify_envelope(&envelope, &hmac_key_b64, &expected_sub)?;

        if !force {
            let conflict = {
                let guard = self.inner.lock();
                guard.store.as_ref().and_then(|current| {
                    let current = &current.base.discipline;
                    (current.id != payload.discipline.id).then(|| KidoConflict {
                        current_discipline_id: current.id,
                        current_discipline_name: current.name.clone(),
                        new_discipline_id: payload.discipline.id,
                        new_discipline_name: payload.discipline.name.clone(),
                    })
                })
            };
            if let Some(conflict) = conflict {
                return Ok(OpenKidoResult {
                    adopted: false,
                    payload,
                    conflict: Some(conflict),
                });
            }
        }

        // The UI applies the effective mode right after (`set_sync_mode`);
        // until then the file is treated as on-prem (no network).
        self.stop_polling();
        let (view, events) = {
            let mut guard = self.inner.lock();
            let (local, mut events) = guard.local_state_for(payload.discipline.id);
            let (store, failed) = match local {
                Some(d) => (d.store, d.failed_posts),
                None => (LocalStore::new(payload.clone()), Vec::new()),
            };
            events.extend(guard.install(store, &failed, SyncMode::Offline));
            events.extend(guard.apply_remote(payload));
            (guard.view(), events)
        };
        emit_all(&app, events);
        self.sync_server(&app);
        Ok(OpenKidoResult {
            adopted: true,
            payload: view.ok_or_else(|| "Kein Wettkampf geladen".to_string())?,
            conflict: None,
        })
    }

    /// Write the loaded discipline **including locally recorded results**
    /// as a signed v4 `.kido` file.
    pub fn export_kido_file(&self, path: String) -> Result<(), String> {
        let (snapshot, hmac_key_b64, sub) = {
            let guard = self.inner.lock();
            let snapshot = guard
                .view()
                .ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
            let account = guard
                .account
                .as_ref()
                .ok_or_else(|| "Nicht gekoppelt".to_string())?;
            (snapshot, account.hmac_key_b64.clone(), account.sub.clone())
        };
        let restamped = kido::restamp_for_export(&snapshot, &sub)?;
        let envelope = kido::build_envelope(&restamped, &hmac_key_b64)?;
        let json = serde_json::to_vec_pretty(&envelope)
            .map_err(|e| format!("envelope serialize failed: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("write failed: {e}"))
    }
}

fn make_client(inner: &Arc<Mutex<CloudInner>>) -> Option<CloudClient> {
    let guard = inner.lock();
    let account = guard.account.as_ref()?;
    CloudClient::new(account.base_url.clone(), account.api_key.clone()).ok()
}

fn emit(app: &AppHandle, event: CloudEvent) {
    let _ = app.emit(CLOUD_EVENT, event);
}

fn emit_all(app: &AppHandle, events: Vec<CloudEvent>) {
    for event in events {
        emit(app, event);
    }
}

fn divergence_event(d: &DivergenceState) -> CloudEvent {
    CloudEvent::DivergenceChanged {
        divergences: d.divergences.clone(),
        unsynced: d.unsynced,
    }
}

fn has_active_run(snapshot: &DisciplinePayload) -> bool {
    snapshot
        .runs
        .iter()
        .any(|r| matches!(r.status, RunStatus::Active))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, RUN_A1, RUN_B1, RUN_C1};
    use crate::cloud::types::{PenaltyKind, PenaltyStatus, Wertungsgruppe};

    fn account() -> PairedAccount {
        PairedAccount {
            base_url: "http://localhost".into(),
            api_key: "k".into(),
            hmac_key_b64: "AAAA".into(),
            sub: "sub-1".into(),
            email: "a@b.c".into(),
            display_name: "A".into(),
            key_id: "kid".into(),
        }
    }

    fn inner_with(mode: SyncMode) -> CloudInner {
        let mut inner = CloudInner {
            account: Some(account()),
            ..Default::default()
        };
        inner.install(LocalStore::new(sample_payload()), &[], mode);
        inner
    }

    fn record_completed(inner: &mut CloudInner, lane: u8, ms: u32) -> Option<PostPlan> {
        let (_, plan, _) = inner
            .record_lane(
                lane,
                &HashMap::new(),
                RunStatus::Completed,
                Some(ms),
                Some("2027-06-12T10:00:00Z".into()),
                Some("2027-06-12T10:00:47Z".into()),
            )
            .unwrap();
        plan
    }

    fn has_divergence_event(events: &[CloudEvent]) -> Option<usize> {
        events.iter().find_map(|e| match e {
            CloudEvent::DivergenceChanged {
                divergences,
                unsynced,
            } => Some(
                divergences.len() + unsynced.penalties + unsynced.lineups + unsynced.hand_times,
            ),
            _ => None,
        })
    }

    // ── payload_equiv ────────────────────────────────────────────────

    #[test]
    fn payload_equiv_ignores_exported_at() {
        let a = sample_payload();
        let mut b = a.clone();
        b.exported_at = "2030-01-01T00:00:00Z".into();
        assert!(payload_equiv(&a, &b));
    }

    #[test]
    fn payload_equiv_detects_lineup_change() {
        let a = sample_payload();
        let mut b = a.clone();
        b.runs[1].lineup.push(Uuid::new_v4());
        assert!(!payload_equiv(&a, &b));
    }

    #[test]
    fn payload_equiv_detects_penalty_content_change() {
        let a = sample_payload();
        let mut b = a.clone();
        b.penalties[0].kind = PenaltyKind::Fv;
        assert!(!payload_equiv(&a, &b));
        let mut c = a.clone();
        c.penalties[0].status = PenaltyStatus::Rejected;
        assert!(!payload_equiv(&a, &c));
    }

    #[test]
    fn payload_equiv_detects_wg_change() {
        let a = sample_payload();
        let mut b = a.clone();
        b.team_entries[0].wg = Some(Wertungsgruppe::Wg1);
        assert!(!payload_equiv(&a, &b));
    }

    #[test]
    fn payload_equiv_detects_current_runs_hand_times_teams_runners() {
        let a = sample_payload();
        let mut b = a.clone();
        b.event.current_run_ids = vec![id(RUN_C1)];
        assert!(!payload_equiv(&a, &b));
        let mut c = a.clone();
        c.runs[0].hand_time_1_ms = Some(47_230);
        assert!(!payload_equiv(&a, &c));
        let mut d = a.clone();
        d.teams[0].name = "X".into();
        assert!(!payload_equiv(&a, &d));
        let mut e = a.clone();
        e.runners[0].birth_date = "2010-01-01".into();
        assert!(!payload_equiv(&a, &e));
        let mut f = a.clone();
        f.discipline.lane_count = 1;
        assert!(!payload_equiv(&a, &f));
    }

    // ── recording ────────────────────────────────────────────────────

    #[test]
    fn on_prem_records_locally_without_post() {
        let mut inner = inner_with(SyncMode::Offline);
        let plan = record_completed(&mut inner, 1, 47_240);
        assert!(plan.is_none(), "on-prem never posts");
        let view = inner.view().unwrap();
        let run = view.runs.iter().find(|r| r.id == id(RUN_A1)).unwrap();
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.original_time_ms, Some(47_240));
    }

    #[test]
    fn live_records_and_plans_post() {
        let mut inner = inner_with(SyncMode::Live);
        let plan = record_completed(&mut inner, 2, 51_000).expect("post planned");
        assert_eq!(plan.run_id, id(RUN_B1));
        assert_eq!(plan.body.run_number, 2);
        assert_eq!(plan.body.original_time_ms, Some(51_000));
        // Local copy is consistent before the POST returns.
        assert_eq!(inner.view().unwrap().runs[1].original_time_ms, Some(51_000));
    }

    #[test]
    fn live_without_account_does_not_post() {
        let mut inner = inner_with(SyncMode::Live);
        inner.account = None;
        assert!(record_completed(&mut inner, 1, 47_240).is_none());
    }

    #[test]
    fn duplicate_post_is_skipped() {
        let mut inner = inner_with(SyncMode::Live);
        let plan = record_completed(&mut inner, 1, 47_240).unwrap();
        // Simulate: POST succeeded but the server row still differs in a
        // timestamp format, so a re-record would create an edit again.
        inner.post_memos.insert(plan.run_id, plan.body.clone());
        assert!(record_completed(&mut inner, 1, 47_240).is_none());
    }

    #[test]
    fn record_on_lane_without_run() {
        let mut inner = inner_with(SyncMode::Offline);
        inner
            .store
            .as_mut()
            .unwrap()
            .base
            .event
            .current_run_ids
            .clear();
        let (run_id, plan, events) = inner
            .record_lane(1, &HashMap::new(), RunStatus::Active, None, None, None)
            .unwrap();
        assert!(run_id.is_none() && plan.is_none() && events.is_empty());
    }

    #[test]
    fn record_without_discipline_is_error() {
        let mut inner = CloudInner::default();
        assert!(inner
            .record_lane(1, &HashMap::new(), RunStatus::Active, None, None, None)
            .is_err());
    }

    #[test]
    fn post_success_acknowledges_edit() {
        let mut inner = inner_with(SyncMode::Live);
        let plan = record_completed(&mut inner, 1, 47_240).unwrap();
        let mut server = inner.view().unwrap().runs[0].clone();
        server.updated_at = Some("2027-06-12T10:00:50Z".into());
        inner.post_succeeded(plan.discipline_id, plan.run_id, &plan.body, server.clone());
        assert!(!inner.store.as_ref().unwrap().has_edits());
        assert_eq!(inner.view().unwrap().runs[0], server);
    }

    #[test]
    fn post_failure_keeps_edit_and_queues_failure() {
        let mut inner = inner_with(SyncMode::Live);
        let plan = record_completed(&mut inner, 1, 47_240).unwrap();
        inner.post_failed(
            plan.discipline_id,
            plan.run_id,
            plan.body.clone(),
            "request failed".into(),
        );
        assert_eq!(inner.failures().len(), 1);
        // A later poll with the old web state must not hide the local time.
        let events = inner.apply_remote(sample_payload());
        assert!(has_divergence_event(&events).is_none());
        assert_eq!(inner.view().unwrap().runs[0].original_time_ms, Some(47_240));
        assert_eq!(inner.failures().len(), 1);
    }

    #[test]
    fn dismiss_failed_drops_edit() {
        let mut inner = inner_with(SyncMode::Live);
        let plan = record_completed(&mut inner, 1, 47_240).unwrap();
        inner.post_failed(plan.discipline_id, plan.run_id, plan.body, "x".into());
        inner.dismiss_failed(id(RUN_A1));
        assert!(inner.failures().is_empty());
        assert_eq!(inner.view().unwrap().runs[0].original_time_ms, None);
    }

    // ── live ⇄ on-prem ───────────────────────────────────────────────

    #[test]
    fn going_live_with_local_results_surfaces_divergence() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        let (start, stop, _) = inner.set_mode(SyncMode::Live, false);
        assert!(start && !stop);
        assert!(inner.needs_divergence_check);

        // First poll: the web still has the run pending.
        let events = inner.apply_remote(sample_payload());
        assert_eq!(has_divergence_event(&events), Some(1));
        assert_eq!(inner.divergence.divergences[0].run_id, id(RUN_A1));
        // Local result NOT overwritten by the polled snapshot.
        assert_eq!(inner.view().unwrap().runs[0].original_time_ms, Some(47_240));
        // Posting is blocked until the operator decides.
        assert!(record_completed(&mut inner, 2, 51_000).is_none());
    }

    #[test]
    fn going_live_without_local_results_has_no_divergence() {
        let mut inner = inner_with(SyncMode::Offline);
        inner.set_mode(SyncMode::Live, false);
        assert!(!inner.needs_divergence_check);
        let events = inner.apply_remote(sample_payload());
        assert!(has_divergence_event(&events).is_none());
    }

    #[test]
    fn going_live_with_results_already_on_web_resolves_silently() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        inner.set_mode(SyncMode::Live, false);
        let mut remote = sample_payload();
        remote.runs[0].status = RunStatus::Completed;
        remote.runs[0].original_time_ms = Some(47_240);
        inner.apply_remote(remote);
        assert!(inner.divergence.is_empty());
        assert!(!inner.store.as_ref().unwrap().has_edits());
    }

    #[test]
    fn upload_payload_and_success_unblock_posting() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        record_completed(&mut inner, 2, 51_000);
        inner.set_mode(SyncMode::Live, false);
        inner.apply_remote(sample_payload());
        assert_eq!(inner.divergence.divergences.len(), 2);
        assert!(!inner.may_post());
        let (disc, payload) = inner.upload_payload().unwrap();
        assert_eq!(payload.runs.len(), 2);
        assert_eq!(payload.runs[0].id, id(RUN_A1));
        assert_eq!(payload.runs[1].original_time_ms, Some(51_000));
        let events = inner.upload_succeeded(disc, &payload);
        assert_eq!(has_divergence_event(&events), Some(0));
        assert!(inner.may_post());
        assert!(!inner.store.as_ref().unwrap().has_edits());
        assert!(inner.store.as_ref().unwrap().last_upload_at.is_some());
        // Local results now are the base.
        assert_eq!(inner.view().unwrap().runs[0].original_time_ms, Some(47_240));
    }

    #[test]
    fn upload_success_for_other_discipline_is_ignored() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        let (_, payload) = inner.upload_payload().unwrap();
        assert!(inner.upload_succeeded(Uuid::new_v4(), &payload).is_empty());
        assert!(inner.store.as_ref().unwrap().has_edits());
    }

    #[test]
    fn going_live_with_unsynced_penalty_surfaces_divergence() {
        let mut inner = inner_with(SyncMode::Offline);
        let token = inner.ensure_onprem_token().unwrap().0;
        let (res, events) = inner.judge_write("sub-1", &token, |store, now| {
            judge::submit_penalty(
                store,
                SubmitStationPenaltyInput {
                    station: StationKind::Knotengestell,
                    run_id: Some(id(RUN_A1)),
                    code: Some("knotengestell.ein_knoten_falsch".into()),
                    client_id: None,
                },
                Uuid::new_v4(),
                now,
            )
        });
        assert!(res.is_ok());
        assert!(events
            .iter()
            .any(|e| matches!(e, CloudEvent::SnapshotChanged { .. })));
        inner.set_mode(SyncMode::Live, false);
        let events = inner.apply_remote(sample_payload());
        assert_eq!(has_divergence_event(&events), Some(1));
        assert_eq!(inner.divergence.unsynced.penalties, 1);
        assert!(inner.divergence.divergences.is_empty());
        assert!(!inner.may_post(), "blocked until uploaded / discarded");
    }

    #[test]
    fn going_live_drops_desk_local_current_runs() {
        let mut inner = inner_with(SyncMode::Offline);
        inner
            .organizer(|store, _, _| {
                crate::local::organizer::set_current_runs(store, vec![id(RUN_C1)])
            })
            .unwrap();
        assert_eq!(
            inner.view().unwrap().event.current_run_ids,
            vec![id(RUN_C1)]
        );
        let (_, _, events) = inner.set_mode(SyncMode::Live, false);
        assert!(events
            .iter()
            .any(|e| matches!(e, CloudEvent::SnapshotChanged { .. })));
        assert_eq!(
            inner.view().unwrap().event.current_run_ids,
            vec![id(RUN_A1), id(RUN_B1)]
        );
    }

    // ── on-prem judge / organizer access ─────────────────────────────

    #[test]
    fn judge_access_requires_owner_token_and_on_prem() {
        let mut inner = inner_with(SyncMode::Offline);
        let token = inner.ensure_onprem_token().unwrap().0;
        assert!(inner.judge_read("sub-1", &token, |_| Ok(())).is_ok());
        assert_eq!(
            inner.judge_read("sub-1", "wrong", |_| Ok(())).unwrap_err(),
            ApiError::Unauthorized
        );
        assert_eq!(
            inner
                .judge_read("other", &token, |_| Ok(()))
                .unwrap_err()
                .status(),
            404
        );
        inner.set_mode(SyncMode::Live, false);
        assert_eq!(
            inner
                .judge_read("sub-1", &token, |_| Ok(()))
                .unwrap_err()
                .status(),
            409
        );
        let empty = CloudInner::default();
        assert_eq!(
            empty
                .judge_read("sub-1", &token, |_| Ok(()))
                .unwrap_err()
                .status(),
            404
        );
    }

    #[test]
    fn judge_write_failure_emits_nothing() {
        let mut inner = inner_with(SyncMode::Offline);
        let token = inner.ensure_onprem_token().unwrap().0;
        let (res, events) = inner.judge_write("sub-1", &token, |_, _| -> ApiResult<()> {
            Err(ApiError::Validation("x".into()))
        });
        assert!(res.is_err());
        assert!(events.is_empty());
        let (res, events) = inner.judge_write("sub-1", "bad", |_, _| Ok(()));
        assert_eq!(res.unwrap_err(), ApiError::Unauthorized);
        assert!(events.is_empty());
    }

    #[test]
    fn token_is_generated_once_and_rotated() {
        let mut inner = inner_with(SyncMode::Offline);
        let (t1, _) = inner.ensure_onprem_token().unwrap();
        let (t1b, events) = inner.ensure_onprem_token().unwrap();
        assert_eq!(t1, t1b);
        assert!(events.is_empty());
        let (t2, _) = inner.rotate_onprem_token().unwrap();
        assert_ne!(t1, t2);
        assert_eq!(inner.onprem_token(), Some(t2.clone()));
        assert_eq!(
            inner.judge_read("sub-1", &t1, |_| Ok(())).unwrap_err(),
            ApiError::Unauthorized,
            "old phones get 401"
        );
        assert!(inner.judge_read("sub-1", &t2, |_| Ok(())).is_ok());
        let mut empty = CloudInner::default();
        assert!(empty.ensure_onprem_token().is_err());
        assert!(empty.rotate_onprem_token().is_err());
    }

    #[test]
    fn invalid_stored_token_is_replaced() {
        let mut inner = inner_with(SyncMode::Offline);
        inner.store.as_mut().unwrap().onprem_token = Some("short".into());
        let (t, _) = inner.ensure_onprem_token().unwrap();
        assert_eq!(t.len(), 32);
    }

    #[test]
    fn token_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let token = {
            let mut inner = CloudInner::default();
            inner.restore(dir.path().to_path_buf());
            inner.install(LocalStore::new(sample_payload()), &[], SyncMode::Offline);
            inner.ensure_onprem_token().unwrap().0
        };
        let mut restarted = CloudInner::default();
        restarted.restore(dir.path().to_path_buf());
        assert_eq!(restarted.onprem_token(), Some(token));
    }

    #[test]
    fn organizer_only_on_prem() {
        let mut inner = inner_with(SyncMode::Live);
        let err = inner.organizer(|_, _, _| Ok(())).unwrap_err();
        assert!(err.contains("On-prem"));
        let mut empty = CloudInner::default();
        assert!(empty
            .organizer(|_, _, _| Ok(()))
            .unwrap_err()
            .contains("Kein Wettkampf"));
    }

    #[test]
    fn organizer_uses_account_email_as_reviewer() {
        let mut inner = inner_with(SyncMode::Offline);
        let (by, events) = inner.organizer(|_, by, _| Ok(by.to_string())).unwrap();
        assert_eq!(by, "a@b.c");
        assert!(events.is_empty(), "no change → no event");
        inner.account = None;
        let (by, _) = inner.organizer(|_, by, _| Ok(by.to_string())).unwrap();
        assert_eq!(by, "Desktop (On-prem)");
    }

    #[test]
    fn organizer_change_is_persisted_and_emitted() {
        let dir = tempfile::tempdir().unwrap();
        let mut inner = CloudInner::default();
        inner.restore(dir.path().to_path_buf());
        inner.install(LocalStore::new(sample_payload()), &[], SyncMode::Offline);
        let (_, events) = inner
            .organizer(|store, _, now| {
                crate::local::organizer::set_hand_times(store, id(RUN_A1), 47_230, 47_245)
                    .map(|_| ())
                    .map(|()| now.to_string())
            })
            .unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, CloudEvent::SnapshotChanged { .. })));
        let mut restarted = CloudInner::default();
        restarted.restore(dir.path().to_path_buf());
        assert_eq!(
            restarted.view().unwrap().runs[0].original_time_ms,
            Some(47_240)
        );
    }

    #[test]
    fn discard_restores_web_state() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        inner.set_mode(SyncMode::Live, false);
        inner.apply_remote(sample_payload());
        inner.discard_local();
        assert!(inner.divergence.is_empty());
        assert_eq!(inner.view().unwrap().runs[0].status, RunStatus::Pending);
        assert!(inner.may_post());
    }

    #[test]
    fn switching_to_on_prem_clears_divergence_and_stops_polling() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        inner.set_mode(SyncMode::Live, false);
        inner.apply_remote(sample_payload());
        let (start, stop, events) = inner.set_mode(SyncMode::Offline, true);
        assert!(!start && stop);
        assert_eq!(has_divergence_event(&events), Some(0));
        // Local result still there.
        assert_eq!(inner.view().unwrap().runs[0].original_time_ms, Some(47_240));
    }

    #[test]
    fn set_live_twice_while_polling_is_noop() {
        let mut inner = inner_with(SyncMode::Live);
        let (start, stop, _) = inner.set_mode(SyncMode::Live, true);
        assert!(!start && !stop);
    }

    #[test]
    fn apply_remote_of_other_discipline_is_ignored() {
        let mut inner = inner_with(SyncMode::Live);
        let mut other = sample_payload();
        other.discipline.id = Uuid::new_v4();
        other.teams.clear();
        assert!(inner.apply_remote(other).is_empty());
        assert_eq!(inner.view().unwrap().teams.len(), 3);
    }

    #[test]
    fn apply_remote_emits_only_on_change() {
        let mut inner = inner_with(SyncMode::Live);
        let mut same = sample_payload();
        same.exported_at = "2030-01-01T00:00:00Z".into();
        assert!(inner.apply_remote(same).is_empty());
        let mut changed = sample_payload();
        changed.penalties[0].status = PenaltyStatus::Rejected;
        let events = inner.apply_remote(changed);
        assert!(events
            .iter()
            .any(|e| matches!(e, CloudEvent::SnapshotChanged { .. })));
    }

    // ── persistence integration ──────────────────────────────────────

    #[test]
    fn restart_restores_local_results() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut inner = CloudInner::default();
            inner.restore(dir.path().to_path_buf());
            inner.install(LocalStore::new(sample_payload()), &[], SyncMode::Offline);
            record_completed(&mut inner, 1, 47_240);
            // "crash": inner dropped without unload
        }
        let mut restarted = CloudInner::default();
        restarted.restore(dir.path().to_path_buf());
        assert!(restarted.startup_warning.is_none());
        assert_eq!(restarted.effective_mode, SyncMode::Offline);
        let view = restarted.view().expect("restored");
        assert_eq!(view.runs[0].status, RunStatus::Completed);
        assert_eq!(view.runs[0].original_time_ms, Some(47_240));
    }

    #[test]
    fn restart_restores_failed_posts() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut inner = CloudInner {
                account: Some(account()),
                ..Default::default()
            };
            inner.restore(dir.path().to_path_buf());
            inner.install(LocalStore::new(sample_payload()), &[], SyncMode::Live);
            let plan = record_completed(&mut inner, 1, 47_240).unwrap();
            inner.post_failed(plan.discipline_id, plan.run_id, plan.body, "offline".into());
        }
        let mut restarted = CloudInner::default();
        restarted.restore(dir.path().to_path_buf());
        assert_eq!(restarted.failures().len(), 1);
        assert_eq!(restarted.failures()[0].original_time_ms, Some(47_240));
    }

    #[test]
    fn unload_keeps_file_but_nothing_restored() {
        let dir = tempfile::tempdir().unwrap();
        let mut inner = CloudInner::default();
        inner.restore(dir.path().to_path_buf());
        inner.install(LocalStore::new(sample_payload()), &[], SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        inner.unload();
        assert!(inner.view().is_none());

        let mut restarted = CloudInner::default();
        restarted.restore(dir.path().to_path_buf());
        assert!(restarted.view().is_none());
        // Selecting the discipline again brings the local result back.
        let (local, _) = restarted.local_state_for(id(crate::cloud::fixtures::DISCIPLINE));
        let local = local.expect("file kept");
        assert_eq!(local.store.view().runs[0].original_time_ms, Some(47_240));
    }

    #[test]
    fn corrupt_current_file_is_quarantined_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let disc = id(crate::cloud::fixtures::DISCIPLINE);
        persist::set_current(dir.path(), Some(disc)).unwrap();
        let path = persist::discipline_file(dir.path(), disc);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"garbage").unwrap();
        let mut inner = CloudInner::default();
        inner.restore(dir.path().to_path_buf());
        assert!(inner.view().is_none());
        assert!(inner.startup_warning.is_some());
        assert!(!path.exists(), "broken file moved aside, not overwritten");
    }
}
