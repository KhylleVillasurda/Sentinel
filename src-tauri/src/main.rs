// main.rs
// SENTINEL entry point.
// Responsibilities: build the Tauri app, initialise shared state,
// spawn background tasks (WS server, network monitor, sync engine),
// and register all Tauri commands.
// Nothing else belongs here — keep it as a wiring file only.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sentinel_lib::{commands, db::Db, network, state::AppState, sync, ws};
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // --- Resolve the platform app data directory ---
            // On Windows: C:\Users\<user>\AppData\Roaming\<app-id>\
            // On macOS:   ~/Library/Application Support/<app-id>/
            // On Linux:   ~/.local/share/<app-id>/
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            // --- Initialise encrypted database ---
            let db =
                Db::open(&app_data_dir).map_err(|e| format!("Failed to open database: {e}"))?;

            // --- Initialise shared state ---
            let app_state = Arc::new(Mutex::new(AppState::new(db)));
            app.manage(app_state.clone());

            // --- Spawn background tasks ---
            // Each task gets its own Arc clone — they share state safely via Mutex.
            // tauri::async_runtime::spawn is the correct Tauri 2 equivalent of tokio::spawn.

            // Phase 2: WebSocket ingestion server (ws://localhost:6767)
            let ws_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                ws::start_server(ws_state).await;
            });

            // Phase 3: Network health monitor (pings every 5s)
            let net_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                network::start_monitor(net_state).await;
            });

            // Phase 4: Sync engine (drains unsynced rows every 10s when Stable)
            let sync_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                sync::start_sync(sync_state).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_network_status,
            commands::get_storage_stats,
            commands::get_connected_devices,
            commands::get_sync_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running sentinel");
}
