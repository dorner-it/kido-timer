mod cloud;
mod commands;
mod dto;
mod file_reader;
mod lanes;
mod local;
mod protocol;
mod results;
mod serial_reader;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .setup(|app| {
            // Best-effort restore of the previously paired account from the
            // OS keychain. Failures are logged and ignored — the user will
            // simply have to re-pair from the UI.
            let state = app.state::<AppState>();
            if let Err(e) = state.cloud.load_from_keychain() {
                eprintln!("cloud: keychain load failed: {e}");
            }
            // Local result store: restore the last loaded discipline incl.
            // results recorded before a crash / restart.
            match app.path().app_data_dir() {
                Ok(dir) => state.cloud.init_storage(dir.join(local::persist::DIR_NAME)),
                Err(e) => eprintln!("local store: no app data dir: {e}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_serial_ports,
            commands::connect_serial,
            commands::start_demo,
            commands::disconnect,
            commands::send_reset,
            commands::current_source,
            commands::write_text_file,
            commands::cloud_identity,
            commands::cloud_pair,
            commands::cloud_clear,
            commands::cloud_list_events,
            commands::cloud_list_disciplines,
            commands::cloud_select_discipline,
            commands::cloud_deselect,
            commands::cloud_snapshot,
            commands::cloud_set_sync_mode,
            commands::cloud_record_lane_status,
            commands::cloud_divergences,
            commands::cloud_resolve_divergence,
            commands::cloud_take_startup_warning,
            commands::cloud_retry_post,
            commands::cloud_clear_failed_post,
            commands::cloud_failed_posts,
            commands::cloud_open_kido,
            commands::cloud_export_kido,
            commands::compute_results,
            commands::kido_catalogue,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KiDo-Timer");
}
