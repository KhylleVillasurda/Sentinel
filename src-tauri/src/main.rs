// main.rs
// SENTINEL entry point.
// Responsibilities: build the Tauri app, initialise shared state,
// spawn background tasks (WS server, network monitor, sync engine),
// and register all Tauri commands.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sentinel_lib::{commands, config::Config, db::Db, network, state::AppState, sync, ws};
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let db = Db::open(&app_data_dir).map_err(|e| e.to_string())?;

            // 1. Load the actual config
            let cfg = Config::load(&app_data_dir).expect("Failed to load Sentinel config file");

            // 2. Pass 'cfg' (NOT Config::default()) into AppState
            let app_state = Arc::new(Mutex::new(AppState::new(db, cfg, app_data_dir)));

            // 3. Register state with Tauri
            app.manage(app_state.clone());

            // --- Spawn background tasks ---

            // WebSocket ingestion server
            let ws_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                ws::start_server(ws_state).await;
            });

            // Network health monitor
            let net_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                network::start_monitor(net_state).await;
            });

            // Sync engine
            let sync_state = app_state.clone();
            let sync_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                sync::start_sync(sync_state, sync_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_network_status,
            commands::get_storage_stats,
            commands::get_connected_devices,
            commands::get_sync_log,
            commands::get_settings,
            commands::save_settings,
            commands::force_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running sentinel");
}
