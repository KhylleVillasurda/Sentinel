use std::sync::{Arc, Mutex};
use tauri::State;

use crate::config::Config;
use crate::db::queries::fetch_unsynced;
use crate::error::SentinelError;
use crate::state::{AppState, NetworkStatus};

// ---------------------------------------------------------------------------
// DTOs — Data Transfer Objects
// ---------------------------------------------------------------------------
// These are the shapes returned to the React frontend via invoke().
// They are deliberately simple — no DB types, no internal enums leak out.
// Serde handles serialization to JSON automatically.

/// Returned by `get_network_status`.
#[derive(serde::Serialize)]
pub struct NetworkStatusDto {
    /// One of: "Unknown", "Stable", "Degraded", "Offline"
    pub status: String,
}

/// Returned by `get_storage_stats`.
#[derive(serde::Serialize)]
pub struct StorageStatsDto {
    pub total_rows: usize,    // was total_count
    pub unsynced_rows: usize, // was unsynced_count
    pub size_kb: u64,
}

/// Returned by `get_sync_log`.
#[derive(serde::Serialize)]
pub struct SyncEventDto {
    pub message: String,
    pub timestamp: i64,
}

/// Returned by `get_config` — mirrors `Config` but as a DTO so the frontend
/// is insulated from internal field type changes.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConfigDto {
    pub cloud_endpoint: String,
    pub sync_interval_secs: u64,
    pub ws_host: String,
    pub ws_port: u16,
    pub log_max_entries: usize,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------
// Rules (from handoff working rules):
//   - Commands stay thin: read state, return DTO, nothing else
//   - No logic lives here — all logic lives in its respective module
//   - State lock is held only long enough to copy what's needed

/// Returns the current network health status.
///
/// Called by `useNetworkStatus.js` every 5 seconds.
#[tauri::command]
pub fn get_network_status(state: State<Arc<Mutex<AppState>>>) -> NetworkStatusDto {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_network_status");
    NetworkStatusDto {
        status: network_status_to_str(&s.network_status).to_string(),
    }
}

/// Returns storage statistics for the dashboard storage bar.
///
/// Called by `useStorageStats.js` every 10 seconds.
#[tauri::command]
pub fn get_storage_stats(state: State<Arc<Mutex<AppState>>>) -> Result<StorageStatsDto, String> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_storage_stats");

    let unsynced = fetch_unsynced(&s.db.conn).map_err(|e| e.to_string())?;
    let unsynced_rows = unsynced.len();

    let total_rows: usize =
        s.db.conn
            .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count payloads: {e}"))?;

    // Get DB file size in KB
    let size_kb: u64 =
        s.db.conn
            .query_row(
                "SELECT page_count * page_size / 1024 FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

    Ok(StorageStatsDto {
        total_rows,
        unsynced_rows,
        size_kb,
    })
}

/// Returns the list of currently connected device IDs.
///
/// Called by `DeviceList.jsx` via `useNetworkStatus.js`.
#[tauri::command]
pub fn get_connected_devices(state: State<Arc<Mutex<AppState>>>) -> Vec<String> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_connected_devices");
    s.connected_devices.clone()
}

/// Returns the rolling sync event log (most recent 100 entries).
///
/// Called by `SyncLog.jsx` to display recent sync activity.
#[tauri::command]
pub fn get_sync_log(state: State<Arc<Mutex<AppState>>>) -> Vec<SyncEventDto> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_sync_log");
    s.sync_log
        .iter()
        .rev() // most recent first
        .map(|e| SyncEventDto {
            message: e.message.clone(),
            timestamp: e.timestamp,
        })
        .collect()
}

#[tauri::command]
pub fn get_settings(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<Config, SentinelError> {
    let s = state
        .lock()
        .map_err(|_| SentinelError::LockPoisoned("State".to_string()))?;
    Ok(s.config.clone())
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, Arc<Mutex<AppState>>>,
    settings: ConfigDto, // Your DTO from the checklist
) -> Result<(), SentinelError> {
    let mut s = state
        .lock()
        .map_err(|e| SentinelError::LockPoisoned(e.to_string()))?;

    // Tech-user validation
    if settings.ws_port == 0 {
        return Err(SentinelError::Other(
            "Port 0 is reserved by the OS. Please choose a port between 1024-65535.".into(),
        ));
    }

    // Update the live config
    s.config.cloud_endpoint = settings.cloud_endpoint;
    s.config.ws_port = settings.ws_port;
    s.config.ws_host = settings.ws_host;

    // Persist to disk immediately
    s.config.save(&s.app_data_dir)?;

    Ok(())
}

/// Returns the current application configuration.
///
/// Called by `Settings.jsx` on mount to populate the settings form.
#[tauri::command]
pub fn get_config(state: State<Arc<Mutex<AppState>>>) -> ConfigDto {
    let s = state.lock().expect("AppState lock poisoned in get_config");
    ConfigDto {
        cloud_endpoint: s.config.cloud_endpoint.clone(),
        sync_interval_secs: s.config.sync_interval_secs,
        ws_host: s.config.ws_host.clone(),
        ws_port: s.config.ws_port,
        log_max_entries: s.config.log_max_entries,
    }
}

/// Saves updated configuration to disk and applies it to the running state.
///
/// Called by `Settings.jsx` on form submit.
/// Returns `Err` if the disk write fails — the frontend shows this as an error.
///
/// Note: Some settings (ws_port, ws_host) take effect on next restart because
/// the WebSocket server bind address is resolved at startup only.
#[tauri::command]
pub fn save_config(state: State<Arc<Mutex<AppState>>>, payload: ConfigDto) -> Result<(), String> {
    let mut s = state.lock().expect("AppState lock poisoned in save_config");

    // Validate before mutating state
    if payload.cloud_endpoint.trim().is_empty() {
        return Err("cloud_endpoint must not be empty".to_string());
    }
    if payload.ws_host.trim().is_empty() {
        return Err("ws_host must not be empty".to_string());
    }
    if payload.ws_port == 0 {
        return Err("ws_port must be a valid port number (1–65535)".to_string());
    }
    if payload.log_max_entries < 10 {
        return Err("log_max_entries must be at least 10".to_string());
    }

    // Apply to in-memory config
    s.config = Config {
        cloud_endpoint: payload.cloud_endpoint.trim().to_string(),
        sync_interval_secs: payload.sync_interval_secs,
        ws_host: payload.ws_host.trim().to_string(),
        ws_port: payload.ws_port,
        log_max_entries: payload.log_max_entries,
    };

    // Persist to disk
    s.config.save(&s.app_data_dir)
}

#[tauri::command]
pub async fn force_sync(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    handle: tauri::AppHandle,
) -> Result<(), crate::error::SentinelError> {
    let state_inner = state.inner().clone();
    // Run in background so the UI doesn't "hiccup"
    tauri::async_runtime::spawn(async move {
        // You'll call your main sync logic function here
        crate::sync::perform_sync(state_inner, handle).await;
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts `NetworkStatus` to a stable string for the frontend.
/// Using a helper keeps the match exhaustive — adding a new variant
/// causes a compile error here rather than a silent frontend bug.
fn network_status_to_str(status: &NetworkStatus) -> &'static str {
    match status {
        NetworkStatus::Unknown => "Unknown",
        NetworkStatus::Stable => "Stable",
        NetworkStatus::Degraded => "Degraded",
        NetworkStatus::Offline => "Offline",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::insert_payload;
    use crate::db::Db;
    use crate::state::SyncEvent;
    use std::fs;

    fn temp_state() -> (Arc<Mutex<AppState>>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "sentinel_commands_test_{:?}_{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir).expect("Db::open must succeed");
        let state = Arc::new(Mutex::new(AppState::new(
            db,
            crate::config::Config::default(),
            std::path::PathBuf::from("test_data"),
        )));
        (state, dir)
    }

    #[test]
    fn network_status_to_str_covers_all_variants() {
        assert_eq!(network_status_to_str(&NetworkStatus::Unknown), "Unknown");
        assert_eq!(network_status_to_str(&NetworkStatus::Stable), "Stable");
        assert_eq!(network_status_to_str(&NetworkStatus::Degraded), "Degraded");
        assert_eq!(network_status_to_str(&NetworkStatus::Offline), "Offline");
    }

    #[test]
    fn storage_stats_counts_correctly() {
        let (state, dir) = temp_state();

        // Insert 3 rows, mark 1 as synced
        {
            let s = state.lock().unwrap();
            insert_payload(&s.db.conn, "d1", b"blob1", 1000).unwrap();
            let id2 = insert_payload(&s.db.conn, "d2", b"blob2", 2000).unwrap();
            insert_payload(&s.db.conn, "d3", b"blob3", 3000).unwrap();
            crate::db::queries::mark_synced(&s.db.conn, id2).unwrap();
        }

        let s = state.lock().unwrap();
        let unsynced = fetch_unsynced(&s.db.conn).unwrap();
        let total: usize =
            s.db.conn
                .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get(0))
                .unwrap();

        assert_eq!(unsynced.len(), 2, "2 rows should be unsynced");
        assert_eq!(total, 3, "total should be 3");

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sync_log_returns_most_recent_first() {
        let (state, dir) = temp_state();

        {
            let mut s = state.lock().unwrap();
            s.sync_log.push(SyncEvent {
                message: "oldest".to_string(),
                timestamp: 1,
            });
            s.sync_log.push(SyncEvent {
                message: "middle".to_string(),
                timestamp: 2,
            });
            s.sync_log.push(SyncEvent {
                message: "newest".to_string(),
                timestamp: 3,
            });
        }

        let s = state.lock().unwrap();
        let log: Vec<SyncEventDto> = s
            .sync_log
            .iter()
            .rev()
            .map(|e| SyncEventDto {
                message: e.message.clone(),
                timestamp: e.timestamp,
            })
            .collect();

        assert_eq!(log[0].message, "newest");
        assert_eq!(log[2].message, "oldest");

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn connected_devices_returns_current_list() {
        let (state, dir) = temp_state();

        {
            let mut s = state.lock().unwrap();
            s.connected_devices.push("sensor-01".to_string());
            s.connected_devices.push("sensor-02".to_string());
        }

        let s = state.lock().unwrap();
        assert_eq!(s.connected_devices.len(), 2);
        assert!(s.connected_devices.contains(&"sensor-01".to_string()));

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
