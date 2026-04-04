// commands.rs
// Tauri invoke() endpoints — the bridge between React and the Rust backend.
//
// Working rules (from handoff):
//   - Commands stay thin: acquire lock → copy what's needed → return DTO
//   - No logic lives here — all logic is in its respective module
//   - Lock is held only long enough to copy fields, never across await points

use std::sync::{Arc, Mutex};
use tauri::State;

use crate::db::queries::fetch_unsynced;
use crate::error::SentinelError;
use crate::state::{AppState, NetworkStatus};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct NetworkStatusDto {
    /// One of: "Unknown", "Stable", "Degraded", "Offline"
    pub status: &'static str,
}

#[derive(serde::Serialize)]
pub struct StorageStatsDto {
    pub total_rows: usize,
    pub unsynced_rows: usize,
    pub size_kb: u64,
}

#[derive(serde::Serialize)]
pub struct SyncEventDto {
    pub message: String,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Returns the current network health status.
/// Called by `useNetworkStatus.js` every 5 seconds.
#[tauri::command]
pub fn get_network_status(
    state: State<Arc<Mutex<AppState>>>,
) -> Result<NetworkStatusDto, SentinelError> {
    let s = state.lock()?;
    Ok(NetworkStatusDto {
        status: network_status_to_str(&s.network_status),
    })
}

/// Returns storage statistics for the dashboard storage bar.
/// Called by `useStorageStats.js` every 10 seconds.
#[tauri::command]
pub fn get_storage_stats(
    state: State<Arc<Mutex<AppState>>>,
) -> Result<StorageStatsDto, SentinelError> {
    let s = state.lock()?;

    let unsynced_rows = fetch_unsynced(&s.db.conn)?.len();

    let total_rows: usize = s
        .db
        .conn
        .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get(0))?;

    let size_kb: u64 = s
        .db
        .conn
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
/// Called by `DeviceList.jsx`.
#[tauri::command]
pub fn get_connected_devices(
    state: State<Arc<Mutex<AppState>>>,
) -> Result<Vec<String>, SentinelError> {
    let s = state.lock()?;
    // connected_devices is now a HashSet — collect into Vec for JSON serialisation.
    // This avoids cloning the entire Vec<String> that the old version did;
    // individual String clones are unavoidable since Tauri needs owned data.
    Ok(s.connected_devices.iter().cloned().collect())
}

/// Returns the rolling sync event log (most recent first, up to 100 entries).
/// Called by `SyncLog.jsx`.
#[tauri::command]
pub fn get_sync_log(state: State<Arc<Mutex<AppState>>>) -> Result<Vec<SyncEventDto>, SentinelError> {
    let s = state.lock()?;
    // sync_log is a VecDeque — .iter().rev() works identically to Vec.
    Ok(s.sync_log
        .iter()
        .rev()
        .map(|e| SyncEventDto {
            message: e.message.clone(),
            timestamp: e.timestamp,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converts `NetworkStatus` to a `'static str` for the frontend.
/// Exhaustive match ensures new variants cause a compile error here,
/// not a silent frontend bug.
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

        {
            let s = state.lock().unwrap();
            insert_payload(&s.db.conn, "d1", b"blob1", 1000).unwrap();
            let id2 = insert_payload(&s.db.conn, "d2", b"blob2", 2000).unwrap();
            insert_payload(&s.db.conn, "d3", b"blob3", 3000).unwrap();
            crate::db::queries::mark_synced(&s.db.conn, id2).unwrap();
        }

        let s = state.lock().unwrap();
        let unsynced = fetch_unsynced(&s.db.conn).unwrap();
        let total: usize = s
            .db
            .conn
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
            // push_back — VecDeque equivalent of Vec::push
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
            // HashSet::insert instead of Vec::push
            s.connected_devices.insert("sensor-01".to_string());
            s.connected_devices.insert("sensor-02".to_string());
        }

        let s = state.lock().unwrap();
        assert_eq!(s.connected_devices.len(), 2);
        assert!(s.connected_devices.contains("sensor-01"));

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
