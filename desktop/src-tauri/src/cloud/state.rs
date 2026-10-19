//! Cloud session: pairing, discipline snapshot, live result posting,
//! polling — on top of the persisted local result store (`crate::local`).
//!
//! The state machine lives in [`CloudInner`] (pure, unit-tested; every
//! mutation returns the events to emit). [`CloudState`] adds the async
//! HTTP calls, the polling task and the Tauri event bridge.

use std::collections::HashMap;
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
    CloudEvent, CloudIdentity, DisciplineListItem, DisciplinePayload, EventSummary, FailedPost,
    KidoConflict, KidoEnvelope, OpenKidoResult, PairedAccount, RunDivergence, RunResultRequest,
    RunStatus, SyncMode,
};
use crate::lanes;
use crate::local::{persist, LocalStore, PersistedDiscipline, RunEdit};

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
struct CloudInner {
    account: Option<PairedAccount>,
    /// Local copy of the selected discipline (`None` = nothing loaded).
    store: Option<LocalStore>,
    /// Sync mode in effect (web default with the desktop override applied;
    /// set by the UI via `cloud_set_sync_mode`).
    effective_mode: SyncMode,
    /// Last body successfully posted per run (dedupe).
    post_memos: HashMap<Uuid, RunResultRequest>,
    failed_posts: HashMap<Uuid, FailureRecord>,
    /// Local results that differ from the web after going live. While
    /// non-empty, nothing is posted until the operator decides.
    divergences: Vec<RunDivergence>,
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
}

fn now_rfc3339() -> String {
    kido::now_rfc3339().unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

impl CloudInner {
    fn view(&self) -> Option<DisciplinePayload> {
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
            && self.divergences.is_empty()
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
        self.needs_divergence_check = mode == SyncMode::Live && store.has_edits();
        self.store = Some(store);
        self.failed_posts = failed.iter().map(FailureRecord::from_failed_post).collect();
        self.post_memos.clear();
        self.effective_mode = mode;
        self.divergences.clear();
        let mut events: Vec<CloudEvent> = self.snapshot_event().into_iter().collect();
        events.push(CloudEvent::FailedPostsChanged {
            failures: self.failures(),
        });
        events.push(CloudEvent::DivergenceChanged {
            divergences: Vec::new(),
        });
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
        let check = self.is_live() && (self.needs_divergence_check || !self.divergences.is_empty());
        let failures_before = self.failures();
        let mut events = Vec::new();

        let store = self.store.as_mut().expect("checked");
        let before = store.view();
        let base_changed = !payload_equiv(&store.base, &remote);
        if base_changed {
            store.adopt_remote(remote);
        }
        let mut new_divergences = None;
        if check {
            let base = store.base.clone();
            store.drop_edits_matching(&base);
            new_divergences = Some(store.divergence(&base));
        }
        let after = store.view();
        let edits_changed = before.runs != after.runs;
        let edit_ids: Vec<Uuid> = store.run_edits.keys().copied().collect();

        if check {
            self.needs_divergence_check = false;
        }
        if let Some(div) = new_divergences {
            if div != self.divergences {
                self.divergences = div.clone();
                events.push(CloudEvent::DivergenceChanged { divergences: div });
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
    fn set_mode(&mut self, mode: SyncMode, polling: bool) -> (bool, bool, Vec<CloudEvent>) {
        let prev = self.effective_mode;
        self.effective_mode = mode;
        let Some(store) = self.store.as_ref() else {
            return (false, polling, Vec::new());
        };
        let mut events = Vec::new();
        match mode {
            SyncMode::Offline => {
                self.needs_divergence_check = false;
                if !self.divergences.is_empty() {
                    self.divergences.clear();
                    events.push(CloudEvent::DivergenceChanged {
                        divergences: Vec::new(),
                    });
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
                if prev != mode {
                    events.extend(self.persist());
                }
                (self.account.is_some(), false, events)
            }
        }
    }

    /// "Lokale Ergebnisse hochladen": unblock posting and return the POSTs.
    fn take_uploads(&mut self) -> (Vec<PostPlan>, Vec<CloudEvent>) {
        let Some(store) = self.store.as_ref() else {
            return (Vec::new(), Vec::new());
        };
        let discipline_id = store.discipline_id();
        let plans = store
            .pending_uploads()
            .into_iter()
            .map(|(run_id, body)| PostPlan {
                discipline_id,
                run_id,
                body,
            })
            .collect();
        self.divergences.clear();
        self.needs_divergence_check = false;
        (
            plans,
            vec![CloudEvent::DivergenceChanged {
                divergences: Vec::new(),
            }],
        )
    }

    /// "Lokale Änderungen verwerfen": the web's state wins.
    fn discard_local(&mut self) -> Vec<CloudEvent> {
        if let Some(store) = self.store.as_mut() {
            store.discard_edits();
        }
        self.failed_posts.clear();
        self.post_memos.clear();
        self.divergences.clear();
        self.needs_divergence_check = false;
        let mut events = vec![
            CloudEvent::DivergenceChanged {
                divergences: Vec::new(),
            },
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
        self.divergences.clear();
        self.needs_divergence_check = false;
        events.push(CloudEvent::FailedPostsChanged {
            failures: Vec::new(),
        });
        events.push(CloudEvent::DivergenceChanged {
            divergences: Vec::new(),
        });
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
}

#[derive(Default)]
pub struct CloudState {
    inner: Arc<Mutex<CloudInner>>,
    poll_task: Mutex<Option<JoinHandle<()>>>,
}

impl CloudState {
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

    pub fn divergences(&self) -> Vec<RunDivergence> {
        self.inner.lock().divergences.clone()
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
        if live {
            self.start_polling(app, id);
        }
        view.ok_or_else(|| "Kein Wettkampf geladen".to_string())
    }

    /// Apply the effective sync mode (web default + desktop override) for
    /// the loaded discipline: on-prem stops all network traffic, live
    /// starts polling (with a divergence check if local results exist).
    pub fn set_sync_mode(&self, app: AppHandle, mode: SyncMode) {
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
        if let (true, Some(id)) = (start, id) {
            self.start_polling(app, id);
        }
    }

    pub fn deselect(&self, app: AppHandle) {
        self.stop_polling();
        let events = self.inner.lock().unload();
        emit_all(&app, events);
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

    /// Resolve a divergence: upload the local results via the result
    /// endpoint (one POST per run) or discard them.
    ///
    /// TODO(on-prem PR 6): upload locally recorded penalties / lineups /
    /// hand times too (needs new endpoints); today only run timing is local.
    pub async fn resolve_divergence(&self, app: AppHandle, upload: bool) -> Result<(), String> {
        if !upload {
            let events = self.inner.lock().discard_local();
            emit_all(&app, events);
            return Ok(());
        }
        let (plans, events) = self.inner.lock().take_uploads();
        emit_all(&app, events);
        let mut failed = 0usize;
        for plan in plans {
            if self
                .do_post(app.clone(), plan.discipline_id, plan.run_id, plan.body)
                .await
                .is_err()
            {
                failed += 1;
            }
        }
        if failed > 0 {
            return Err(format!(
                "{failed} Ergebnis(se) konnten nicht hochgeladen werden — siehe „Nicht übertragene Ergebnisse“"
            ));
        }
        Ok(())
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
            CloudEvent::DivergenceChanged { divergences } => Some(divergences.len()),
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
        b.approved_penalties[0].kind = PenaltyKind::Fv;
        assert!(!payload_equiv(&a, &b));
        let mut c = a.clone();
        c.approved_penalties[0].status = PenaltyStatus::Rejected;
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
        assert_eq!(inner.divergences[0].run_id, id(RUN_A1));
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
        assert!(inner.divergences.is_empty());
        assert!(!inner.store.as_ref().unwrap().has_edits());
    }

    #[test]
    fn upload_unblocks_and_returns_posts() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        record_completed(&mut inner, 2, 51_000);
        inner.set_mode(SyncMode::Live, false);
        inner.apply_remote(sample_payload());
        assert_eq!(inner.divergences.len(), 2);
        let (plans, events) = inner.take_uploads();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].run_id, id(RUN_A1));
        assert_eq!(plans[1].body.original_time_ms, Some(51_000));
        assert_eq!(has_divergence_event(&events), Some(0));
        assert!(inner.may_post());
    }

    #[test]
    fn discard_restores_web_state() {
        let mut inner = inner_with(SyncMode::Offline);
        record_completed(&mut inner, 1, 47_240);
        inner.set_mode(SyncMode::Live, false);
        inner.apply_remote(sample_payload());
        inner.discard_local();
        assert!(inner.divergences.is_empty());
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
        changed.approved_penalties[0].status = PenaltyStatus::Rejected;
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
