use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::cloud::types::{
    CloudIdentity, DisciplineListItem, DisciplinePayload, EventSummary, FailedPost, OpenKidoResult,
    RunResultRequest, RunStatus,
};
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
) -> Result<DisciplinePayload, String> {
    state.cloud.select_discipline(app_handle, id).await
}

#[tauri::command]
pub fn cloud_deselect(app_handle: AppHandle, state: State<'_, AppState>) {
    state.cloud.deselect(app_handle);
}

#[tauri::command]
pub fn cloud_snapshot(state: State<'_, AppState>) -> Option<DisciplinePayload> {
    state.cloud.current_snapshot()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cloud_post_run_status(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    discipline_id: Uuid,
    run_id: Uuid,
    run_number: u32,
    status: RunStatus,
    original_time_ms: Option<u32>,
    started_at: Option<String>,
    ended_at: Option<String>,
) -> Result<bool, String> {
    let body = RunResultRequest {
        run_number,
        status,
        original_time_ms,
        started_at,
        ended_at,
    };
    state
        .cloud
        .maybe_post_run_status(app_handle, discipline_id, run_id, body)
        .await
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
pub fn cloud_clear_failed_post(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    run_id: Uuid,
) {
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
