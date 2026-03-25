use std::sync::{Arc, Mutex};
use tauri::State;

use crate::db::queries::fetch_unsynced;
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
    /// Number of payload rows not yet synced to the cloud
    pub unsynced_count: usize,
    /// Total number of payload rows in the DB
    pub total_count: usize,
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
pub fn get_storage_stats(state: State<Arc<Mutex<AppState>>>) -> Result<StorageStatsDto, String> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_storage_stats");

    let unsynced = fetch_unsynced(&s.db.conn)?;
    let unsynced_count = unsynced.len();

    let total_count: usize =
        s.db.conn
            .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get(0))
            .map_err(|e| format!("Failed to count payloads: {e}"))?;

    Ok(StorageStatsDto {
        unsynced_count,
        total_count,
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
        let state = Arc::new(Mutex::new(AppState::new(db)));
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
