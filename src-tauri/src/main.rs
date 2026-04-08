// main.rs
// SENTINEL entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sentinel_lib::{commands, network, state::AppState, sync, ws};
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use tauri::Manager;
use tokio::time::{interval, Duration};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let db = sentinel_lib::db::Db::open(&app_data_dir)?;

            let cfg = sentinel_lib::config::Config::load(&app_data_dir)
                .expect("Failed to load Sentinel config");

            let app_state = Arc::new(Mutex::new(AppState::new(db, cfg, app_data_dir)));
            
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

            // Pairing Mode Auto-Expiry Task
            let pairing_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                let mut ticker = interval(Duration::from_secs(1));
                loop {
                    ticker.tick().await;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    
                    let s = pairing_state.lock().unwrap();
                    let expiry = s.pairing_expiry.load(Ordering::SeqCst);
                    
                    if expiry > 0 && now >= expiry {
                        println!("[auth] Pairing mode expired.");
                        s.pairing_mode.store(false, Ordering::SeqCst);
                        s.pairing_expiry.store(0, Ordering::SeqCst);
                    }
                }
            });

            app.manage(app_state);
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
            commands::is_logging_enabled,
            commands::set_logging_enabled,
            commands::get_log_buffer,
            commands::toggle_pairing_mode,
            commands::is_pairing_mode_active,
            commands::get_pairing_expiry,
            commands::get_registered_devices,
            commands::revoke_device,
            commands::get_decrypted_payloads,
        ])
        .run(tauri::generate_context!())
        .expect("error while running sentinel");
}
