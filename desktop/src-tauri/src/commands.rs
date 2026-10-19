use std::collections::HashMap;

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::cloud::state::{RecordOutcome, UploadOutcome};
use crate::cloud::types::{
    CloudIdentity, DisciplineListItem, DisciplinePayload, DivergenceState, EventSummary,
    FailedPost, OpenKidoResult, Penalty, RunStatus, SyncMode,
};
use crate::local::organizer;
use crate::local::server::OnPremStatus;
use crate::results::{CatalogueItem, ResultsResponse};
use crate::state::AppState;

#[tauri::command]
pub fn list_serial_ports() -> Vec<String> {
    crate::serial_reader::list_ports()
}

#[tauri::command]
pub fn connect_serial(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    port: String,
    baud: u32,
) -> Result<(), String> {
    state.connect_serial(app_handle, port, baud)
}

#[tauri::command]
pub fn start_demo(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    path: String,
    speed: f64,
) -> Result<(), String> {
    state.start_demo(app_handle, path, speed)
}

#[tauri::command]
pub fn disconnect(state: State<'_, AppState>) {
    state.disconnect();
}

#[tauri::command]
pub fn send_reset(state: State<'_, AppState>) -> Result<(), String> {
    state.send_reset()
}

#[tauri::command]
pub fn current_source(state: State<'_, AppState>) -> Option<String> {
    state
        .connection
        .lock()
        .as_ref()
        .map(|c| c.source().to_string())
}

#[tauri::command]
pub fn write_text_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents.as_bytes()).map_err(|e| format!("{}: {}", path, e))
}

// ─── Cloud commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn cloud_identity(state: State<'_, AppState>) -> Option<CloudIdentity> {
    state.cloud.current_identity()
}

#[tauri::command]
pub async fn cloud_pair(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    base_url: String,
    api_key: String,
) -> Result<CloudIdentity, String> {
    state.cloud.pair(app_handle, base_url, api_key).await
}

#[tauri::command]
pub fn cloud_clear(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.cloud.clear(app_handle)
}

#[tauri::command]
pub async fn cloud_list_events(state: State<'_, AppState>) -> Result<Vec<EventSummary>, String> {
    state.cloud.list_events().await
}

#[tauri::command]
pub async fn cloud_list_disciplines(
    state: State<'_, AppState>,
) -> Result<Vec<DisciplineListItem>, String> {
    state.cloud.list_disciplines().await
}

#[tauri::command]
pub async fn cloud_select_discipline(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    mode: Option<SyncMode>,
) -> Result<DisciplinePayload, String> {
    state.cloud.select_discipline(app_handle, id, mode).await
}

#[tauri::command]
pub fn cloud_deselect(app_handle: AppHandle, state: State<'_, AppState>) {
    state.cloud.deselect(app_handle);
}

#[tauri::command]
pub fn cloud_snapshot(state: State<'_, AppState>) -> Option<DisciplinePayload> {
    state.cloud.current_snapshot()
}

/// Effective sync mode (web default + desktop override) for the loaded
/// discipline. `offline` = on-prem (starts the judge server); `live`
/// switches the web to live first if needed (A2).
#[tauri::command]
pub async fn cloud_set_sync_mode(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    mode: SyncMode,
) -> Result<(), String> {
    state.cloud.set_sync_mode(app_handle, mode).await;
    Ok(())
}

/// Record a timing change on desktop lane `lane` (1-based = timer
/// channel). Stored locally + persisted; posted when live.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cloud_record_lane_status(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    lane: u8,
    overrides: Option<HashMap<u8, Uuid>>,
    status: RunStatus,
    original_time_ms: Option<u32>,
    started_at: Option<String>,
    ended_at: Option<String>,
) -> Result<RecordOutcome, String> {
    state
        .cloud
        .record_lane_status(
            app_handle,
            lane,
            overrides.unwrap_or_default(),
            status,
            original_time_ms,
            started_at,
            ended_at,
        )
        .await
}

#[tauri::command]
pub fn cloud_divergences(state: State<'_, AppState>) -> DivergenceState {
    state.cloud.divergences()
}

/// `upload = true`: upload all local changes (A3); `false`: discard them.
#[tauri::command]
pub async fn cloud_resolve_divergence(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    upload: bool,
) -> Result<(), String> {
    state.cloud.resolve_divergence(app_handle, upload).await
}

#[tauri::command]
pub fn cloud_take_startup_warning(state: State<'_, AppState>) -> Option<String> {
    state.cloud.take_startup_warning()
}

#[tauri::command]
pub async fn cloud_retry_post(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_id: Uuid,
) -> Result<(), String> {
    state.cloud.retry_post(app_handle, run_id).await
}

#[tauri::command]
pub fn cloud_clear_failed_post(app_handle: AppHandle, state: State<'_, AppState>, run_id: Uuid) {
    state.cloud.clear_failed_post(app_handle, run_id);
}

#[tauri::command]
pub fn cloud_failed_posts(state: State<'_, AppState>) -> Vec<FailedPost> {
    state.cloud.list_failed_posts()
}

#[tauri::command]
pub fn cloud_open_kido(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    path: String,
    force: Option<bool>,
) -> Result<OpenKidoResult, String> {
    state
        .cloud
        .open_kido_file(app_handle, path, force.unwrap_or(false))
}

#[tauri::command]
pub fn cloud_export_kido(state: State<'_, AppState>, path: String) -> Result<(), String> {
    state.cloud.export_kido_file(path)
}

// ─── Results / catalogue (kido-core) ─────────────────────────────────────────

/// WG rankings + Tagessieger of the loaded discipline (local view incl.
/// unsynced results), computed with `kido_core::scoring`.
#[tauri::command]
pub fn compute_results(state: State<'_, AppState>) -> Result<ResultsResponse, String> {
    let snapshot = state
        .cloud
        .current_snapshot()
        .ok_or_else(|| "Kein Wettkampf geladen".to_string())?;
    crate::results::compute(&snapshot)
}

#[tauri::command]
pub fn kido_catalogue() -> Vec<CatalogueItem> {
    crate::results::catalogue()
}

/// Hand-time mean (1/100 s, half up) — `kido_core::timing::hand_time_mean`.
#[tauri::command]
pub fn hand_time_mean(hand_time_1_ms: u32, hand_time_2_ms: u32) -> u32 {
    kido_core::timing::hand_time_mean(hand_time_1_ms, hand_time_2_ms)
}

// ─── On-prem (local judge server + organizer) ───────────────────────────────

#[tauri::command]
pub fn onprem_status(state: State<'_, AppState>) -> OnPremStatus {
    state.cloud.onprem_status()
}

/// "Neuer Zugangscode": old phones get 401 and must rescan.
#[tauri::command]
pub fn onprem_rotate_token(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<OnPremStatus, String> {
    state.cloud.rotate_onprem_token(&app_handle)
}

/// Current runs, one per lane (max `lane_count`).
#[tauri::command]
pub fn onprem_set_current_runs(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_ids: Vec<Uuid>,
) -> Result<(), String> {
    state.cloud.organizer(&app_handle, |store, _, _| {
        organizer::set_current_runs(store, run_ids)
    })
}

/// Bestätigen (`approve = true`) / Zurücknehmen of a pending error.
#[tauri::command]
pub fn onprem_review_penalty(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    penalty_id: Uuid,
    approve: bool,
) -> Result<Penalty, String> {
    state.cloud.organizer(&app_handle, |store, by, now| {
        organizer::review_penalty(store, penalty_id, approve, by, now)
    })
}

/// "Rückgängig": a decided error → pending again (until "Übernehmen").
#[tauri::command]
pub fn onprem_undo_review(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    penalty_id: Uuid,
) -> Result<Penalty, String> {
    state.cloud.organizer(&app_handle, |store, _, _| {
        organizer::undo_review(store, penalty_id)
    })
}

/// Penalties whose decision comes from the web snapshot — the UI hides
/// "Rückgängig" for them.
#[tauri::command]
pub fn onprem_web_decided_penalties(state: State<'_, AppState>) -> Vec<Uuid> {
    state.cloud.web_decided_penalty_ids()
}

/// "Übernehmen": remaining pending errors of the run → approved.
#[tauri::command]
pub fn onprem_take_over(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_id: Uuid,
) -> Result<usize, String> {
    state.cloud.organizer(&app_handle, |store, by, now| {
        organizer::take_over(store, run_id, by, now)
    })
}

#[tauri::command]
pub fn onprem_set_lineup(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_id: Uuid,
    runner_ids: Vec<Uuid>,
) -> Result<(), String> {
    state.cloud.organizer(&app_handle, |store, _, now| {
        organizer::set_lineup(store, run_id, runner_ids, now)
    })
}

/// Returns the stored Handzeit (mean).
#[tauri::command]
pub fn onprem_set_hand_times(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_id: Uuid,
    hand_time_1_ms: u32,
    hand_time_2_ms: u32,
) -> Result<u32, String> {
    state.cloud.organizer(&app_handle, |store, _, _| {
        organizer::set_hand_times(store, run_id, hand_time_1_ms, hand_time_2_ms)
    })
}

#[tauri::command]
pub fn onprem_clear_hand_times(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_id: Uuid,
) -> Result<(), String> {
    state.cloud.organizer(&app_handle, |store, _, _| {
        organizer::clear_hand_times(store, run_id)
    })
}

/// "Ins Web hochladen" (A3).
#[tauri::command]
pub async fn cloud_upload_onprem(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<UploadOutcome, String> {
    state.cloud.upload_onprem(app_handle).await
}
