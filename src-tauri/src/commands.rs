use std::sync::{Arc, Mutex};
use tauri::State;

use crate::config::Config;
use crate::db::queries::fetch_unsynced;
use crate::error::SentinelError;
use crate::state::{AppState, NetworkStatus};

// ---------------------------------------------------------------------------
// DTOs — Data Transfer Objects
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct NetworkStatusDto {
    pub status: String,
}

#[derive(serde::Serialize)]
pub struct StorageStatsDto {
    pub total_rows: usize,
    pub unsynced_rows: usize,
    pub size_kb: u64,
}

#[derive(serde::Serialize)]
pub struct LogEventDto {
    pub timestamp: i64,
    pub level: String,
    pub subsystem: String,
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct SyncEventDto {
    pub message: String,
    pub timestamp: i64,
}

#[derive(serde::Serialize)]
pub struct DecryptedPayloadDto {
    pub id: i64,
    pub device_id: String,
    pub decrypted_data: String,
    pub received_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConfigDto {
    pub cloud_endpoint: String,
    pub sync_interval_secs: u64,
    pub ws_host: String,
    pub ws_port: u16,
    pub log_max_entries: usize,
}

use std::sync::atomic::Ordering;
use crate::db::queries::{list_devices, delete_device, DeviceRow, fetch_recent_payloads};
use crate::crypto::{load_or_create_key, decrypt_payload};

#[tauri::command]
pub fn get_decrypted_payloads(state: State<Arc<Mutex<AppState>>>, limit: usize) -> Result<Vec<DecryptedPayloadDto>, String> {
    let s = state.lock().expect("AppState lock poisoned");
    let key = load_or_create_key();
    
    let rows = fetch_recent_payloads(&s.db.conn, limit).map_err(|e| e.to_string())?;
    
    let mut decrypted = Vec::new();
    for row in rows {
        let data = match decrypt_payload(&row.encrypted_blob, &key) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(e) => format!("[Decryption Error: {}]", e),
        };
        
        decrypted.push(DecryptedPayloadDto {
            id: row.id,
            device_id: row.device_id,
            decrypted_data: data,
            received_at: row.received_at,
        });
    }
    
    Ok(decrypted)
}

#[tauri::command]
pub fn toggle_pairing_mode(state: State<Arc<Mutex<AppState>>>, active: bool) -> i64 {
    let s = state.lock().expect("AppState lock poisoned");
    s.pairing_mode.store(active, Ordering::SeqCst);
    
    let expiry = if active {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let exp = now + 60; // 60 seconds window
        s.pairing_expiry.store(exp, Ordering::SeqCst);
        exp
    } else {
        s.pairing_expiry.store(0, Ordering::SeqCst);
        0
    };
    expiry
}

#[tauri::command]
pub fn is_pairing_mode_active(state: State<Arc<Mutex<AppState>>>) -> bool {
    let s = state.lock().expect("AppState lock poisoned");
    s.pairing_mode.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn get_pairing_expiry(state: State<Arc<Mutex<AppState>>>) -> i64 {
    let s = state.lock().expect("AppState lock poisoned");
    s.pairing_expiry.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn get_registered_devices(state: State<Arc<Mutex<AppState>>>) -> Result<Vec<DeviceRow>, String> {
    let s = state.lock().expect("AppState lock poisoned");
    list_devices(&s.db.conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn revoke_device(state: State<Arc<Mutex<AppState>>>, device_id: String) -> Result<(), String> {
    let s = state.lock().expect("AppState lock poisoned");
    delete_device(&s.db.conn, &device_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn is_logging_enabled(state: State<Arc<Mutex<AppState>>>) -> bool {
    let s = state.lock().expect("AppState lock poisoned");
    s.logging.is_enabled()
}

#[tauri::command]
pub fn set_logging_enabled(state: State<Arc<Mutex<AppState>>>, enabled: bool) {
    let s = state.lock().expect("AppState lock poisoned");
    s.logging.set_enabled(enabled);
}

#[tauri::command]
pub fn get_log_buffer(state: State<Arc<Mutex<AppState>>>) -> Vec<LogEventDto> {
    let s = state.lock().expect("AppState lock poisoned");
    let buffer = s.logging.buffer.lock().unwrap();
    buffer
        .iter()
        .map(|e| LogEventDto {
            timestamp: e.timestamp,
            level: format!("{:?}", e.level).to_lowercase(),
            subsystem: format!("{:?}", e.subsystem),
            message: e.message.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn get_network_status(state: State<Arc<Mutex<AppState>>>) -> NetworkStatusDto {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_network_status");
    NetworkStatusDto {
        status: network_status_to_str(&s.network_status).to_string(),
    }
}

#[tauri::command]
pub fn get_storage_stats(state: State<Arc<Mutex<AppState>>>) -> Result<StorageStatsDto, String> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_storage_stats");

    let unsynced = fetch_unsynced(&s.db.conn).map_err(|e| e.to_string())?;
    let unsynced_rows = unsynced.len();

    let total_rows: usize =
        s.db.conn
            .query_row("SELECT COUNT(*) FROM payloads", [], |row| row.get::<_, usize>(0))
            .map_err(|e| format!("Failed to count payloads: {e}"))?;

    let size_kb: u64 =
        s.db.conn
            .query_row(
                "SELECT page_count * page_size / 1024 FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| row.get::<_, u64>(0),
            )
            .unwrap_or(0);

    Ok(StorageStatsDto {
        total_rows,
        unsynced_rows,
        size_kb,
    })
}

#[tauri::command]
pub fn get_connected_devices(state: State<Arc<Mutex<AppState>>>) -> Vec<String> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_connected_devices");
    s.connected_devices.clone()
}

#[tauri::command]
pub fn get_sync_log(state: State<Arc<Mutex<AppState>>>) -> Vec<SyncEventDto> {
    let s = state
        .lock()
        .expect("AppState lock poisoned in get_sync_log");
    let sync_log = s.logging.legacy_sync_log.lock().unwrap();
    sync_log
        .iter()
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
    settings: ConfigDto,
) -> Result<(), SentinelError> {
    let mut s = state
        .lock()
        .map_err(|e| SentinelError::LockPoisoned(e.to_string()))?;

    if settings.ws_port == 0 {
        return Err(SentinelError::Other(
            "Port 0 is reserved by the OS. Please choose a port between 1024-65535.".into(),
        ));
    }

    s.config.cloud_endpoint = settings.cloud_endpoint;
    s.config.ws_port = settings.ws_port;
    s.config.ws_host = settings.ws_host;
    s.config.save(&s.app_data_dir)?;

    Ok(())
}

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

#[tauri::command]
pub fn save_config(state: State<Arc<Mutex<AppState>>>, payload: ConfigDto) -> Result<(), String> {
    let mut s = state.lock().expect("AppState lock poisoned in save_config");

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

    s.config = Config {
        cloud_endpoint: payload.cloud_endpoint.trim().to_string(),
        sync_interval_secs: payload.sync_interval_secs,
        ws_host: payload.ws_host.trim().to_string(),
        ws_port: payload.ws_port,
        log_max_entries: payload.log_max_entries,
    };

    s.config.save(&s.app_data_dir)
}

#[tauri::command]
pub async fn force_sync(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    handle: tauri::AppHandle,
) -> Result<(), crate::error::SentinelError> {
    let state_inner = state.inner().clone();
    let logging = {
        let s = state_inner.lock().unwrap();
        s.logging.clone()
    };
    tauri::async_runtime::spawn(async move {
        crate::sync::perform_sync(state_inner, logging, handle).await;
    });
    Ok(())
}

fn network_status_to_str(status: &NetworkStatus) -> &'static str {
    match status {
        NetworkStatus::Unknown => "Unknown",
        NetworkStatus::Stable => "Stable",
        NetworkStatus::Degraded => "Degraded",
        NetworkStatus::Offline => "Offline",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::insert_payload;
    use crate::db::Db;
    use crate::logging::SyncEvent;
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
            let s = state.lock().unwrap();
            let mut sync_log = s.logging.legacy_sync_log.lock().unwrap();
            // LogManager::log uses insert(0, ...) so it's LIFO (newest at index 0)
            sync_log.insert(0, SyncEvent {
                message: "oldest".to_string(),
                timestamp: 1,
            });
            sync_log.insert(0, SyncEvent {
                message: "middle".to_string(),
                timestamp: 2,
            });
            sync_log.insert(0, SyncEvent {
                message: "newest".to_string(),
                timestamp: 3,
            });
        }

        let s = state.lock().unwrap();
        let log: Vec<SyncEventDto> = {
            let sync_log = s.logging.legacy_sync_log.lock().unwrap();
            sync_log
                .iter()
                .map(|e| SyncEventDto {
                    message: e.message.clone(),
                    timestamp: e.timestamp,
                })
                .collect()
        };

        // newest should be at index 0, oldest at index 2
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
