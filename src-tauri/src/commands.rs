use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

use crate::db::queries::fetch_unsynced;
use crate::error::SentinelError;
use crate::state::{AppState, NetworkStatus};

// ---------------------------------------------------------------------------
// DTOs — Data Transfer Objects
// ---------------------------------------------------------------------------
// These are the shapes returned to the React frontend via invoke().
// They are deliberately simple — no DB types, no internal enums leak out.
// Serde handles serialization to JSON automatically.

/// Returned by / accepted by `get_settings` and `save_settings`.
/// Mirrors `Settings` exactly — a separate DTO keeps internal types out of the API surface.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SettingsDto {
    pub cloud_endpoint: String,
    pub ws_bind_address: String,
}

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
pub fn get_storage_stats(
    state: State<Arc<Mutex<AppState>>>,
) -> Result<StorageStatsDto, SentinelError> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_storage_stats");

    let unsynced = fetch_unsynced(&s.db.conn)?;
    let unsynced_rows = unsynced.len();

    let total_rows: usize =
        s.db.conn
            .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get(0))?;

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
    // HashSet → Vec so the frontend receives a stable JSON array
    s.connected_devices.iter().cloned().collect()
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

/// Returns the current settings so the React UI can populate the Settings panel.
#[tauri::command]
pub fn get_settings(state: State<Arc<Mutex<AppState>>>) -> SettingsDto {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_settings");
    SettingsDto {
        cloud_endpoint: s.settings.cloud_endpoint.clone(),
        ws_bind_address: s.settings.ws_bind_address.clone(),
    }
}

/// Saves new settings to disk and updates the live AppState so sync.rs and
/// ws.rs pick up the new values on their next cycle.
///
/// NOTE: The WebSocket server bind address takes effect only after restart
/// (rebinding a live TCP socket requires stopping the listener task).
/// The cloud endpoint takes effect immediately on the next sync cycle.
#[tauri::command]
pub fn save_settings(
    state: State<Arc<Mutex<AppState>>>,
    app: tauri::AppHandle,
    dto: SettingsDto,
) -> Result<(), SentinelError> {
    let settings = crate::settings::Settings {
        cloud_endpoint: dto.cloud_endpoint,
        ws_bind_address: dto.ws_bind_address,
    };

    // Persist to disk first — if the write fails, do not update live state
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| SentinelError::Io(format!("Could not resolve app data dir: {e}")))?;

    settings.save(&app_data_dir)?;

    // Update live state so sync.rs picks up the new endpoint immediately
    let mut s = state
        .lock()
        .expect("AppState lock poisoned in save_settings");
    s.settings = settings;

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
            crate::settings::Settings::default(),
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
            s.sync_log.push_back(SyncEvent {
                message: "oldest".to_string(),
                timestamp: 1,
            });
            s.sync_log.push_back(SyncEvent {
                message: "middle".to_string(),
                timestamp: 2,
            });
            s.sync_log.push_back(SyncEvent {
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
            s.connected_devices.insert("sensor-01".to_string());
            s.connected_devices.insert("sensor-02".to_string());
        }

        let s = state.lock().unwrap();
        assert_eq!(s.connected_devices.len(), 2);
        assert!(s.connected_devices.contains(&"sensor-01".to_string()));

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn get_settings_returns_default_values() {
        let (state, dir) = temp_state();

        let s = state.lock().unwrap();
        assert_eq!(s.settings.cloud_endpoint, "http://127.0.0.1:9000/ingest");
        assert_eq!(s.settings.ws_bind_address, "0.0.0.0:6767");

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_update_reflects_in_state() {
        let (state, dir) = temp_state();

        let new_settings = crate::settings::Settings {
            cloud_endpoint: "http://10.0.0.5:9000/ingest".to_string(),
            ws_bind_address: "0.0.0.0:7777".to_string(),
        };

        {
            let mut s = state.lock().unwrap();
            s.settings = new_settings.clone();
        }

        let s = state.lock().unwrap();
        assert_eq!(s.settings.cloud_endpoint, "http://10.0.0.5:9000/ingest");
        assert_eq!(s.settings.ws_bind_address, "0.0.0.0:7777");

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
