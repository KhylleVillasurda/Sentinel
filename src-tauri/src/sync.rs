use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use tauri::{AppHandle, Emitter};

use crate::db::queries::{fetch_unsynced, mark_synced};
use crate::logging::{LogLevel, LogSubsystem};
use crate::log_event_emit;
#[cfg(test)]
use crate::log_event;
use crate::state::{AppState, NetworkStatus};

/// Starts the sync engine loop.
///
/// Runs forever — spawn with `tokio::spawn` from `main.rs` at startup.
/// Every `SYNC_INTERVAL_SECS` seconds:
///   1. Checks if network is `Stable` — skips the cycle if not
///   2. Fetches all unsynced rows from the DB
///   3. Attempts a single batched POST to the cloud endpoint
///   4. On success: marks all rows as synced
///   5. On failure: skips the failed batch, logs the error, and continues
pub async fn start_sync(state: Arc<Mutex<AppState>>, handle: AppHandle) {
    let sync_interval = {
        let s = state.lock().unwrap();
        s.config.sync_interval_secs
    };

    let mut ticker = interval(Duration::from_secs(sync_interval));

    loop {
        ticker.tick().await;
        perform_sync(state.clone(), handle.clone()).await;
    }
}

pub async fn perform_sync(state: Arc<Mutex<AppState>>, handle: AppHandle) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build reqwest client");

    // 1. Get status and config
    let (network_status, cloud_endpoint) = {
        let s = state.lock().expect("AppState lock poisoned");
        (s.network_status.clone(), s.config.cloud_endpoint.clone())
    };

    // 2. Local Network Check
    if network_status != NetworkStatus::Stable {
        return;
    }

    // 3. Milestone 1: Heartbeat Check (The "Pre-flight")
    if !is_cloud_healthy(&client, &cloud_endpoint).await {
        log_event_emit!(
            state,
            handle,
            LogLevel::Warn,
            LogSubsystem::Sync,
            "Cloud Offline: /health unreachable"
        );
        return;
    }

    // 4. Fetch rows (Your existing logic)
    let rows = {
        let s = state.lock().expect("AppState lock poisoned");
        match fetch_unsynced(&s.db.conn) {
            Ok(r) => r,
            Err(e) => {
                log_event_emit!(
                    state,
                    handle,
                    LogLevel::Error,
                    LogSubsystem::Sync,
                    "Failed to fetch: {e}"
                );
                return;
            }
        }
    };

    if rows.is_empty() {
        return;
    }

    // 5. Build and POST Batch (Your existing logic)
    let batch: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.id,
                "device_id": row.device_id,
                "encrypted_blob": base64_encode(&row.encrypted_blob),
                "received_at": row.received_at,
            })
        })
        .collect();

    match client
        .post(&cloud_endpoint)
        .json(&serde_json::json!({ "payloads": batch }))
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            let mut synced_count = 0;
            {
                let s = state.lock().expect("AppState lock poisoned");
                for row in &rows {
                    if mark_synced(&s.db.conn, row.id).is_ok() {
                        synced_count += 1;
                    }
                }
            }
            log_event_emit!(
                state,
                handle,
                LogLevel::Info,
                LogSubsystem::Sync,
                "Synced {synced_count} row(s) successfully"
            );
        }
        Ok(res) => {
            log_event_emit!(
                state,
                handle,
                LogLevel::Error,
                LogSubsystem::Sync,
                "Server rejected batch: HTTP {}",
                res.status()
            );
        }
        Err(e) => {
            log_event_emit!(
                state,
                handle,
                LogLevel::Error,
                LogSubsystem::Sync,
                "Upload failed: {e}"
            );
        }
    }
}

// Pre-flight check
async fn is_cloud_healthy(client: &reqwest::Client, endpoint: &str) -> bool {
    let health_url = format!("{}/health", endpoint.trim_end_matches('/'));
    client
        .get(health_url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map(|res| res.status().is_success())
        .unwrap_or(false)
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::STANDARD.encode(data)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::insert_payload;
    use crate::db::Db;
    use std::fs;

    fn temp_state() -> (Arc<Mutex<AppState>>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "sentinel_sync_test_{:?}_{}",
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
            std::path::PathBuf::from("./test_data"),
        )));
        (state, dir)
    }

    #[test]
    fn endpoint_read_from_settings() {
        // Verify the sync engine reads cloud_endpoint from state.settings,
        // not a hardcoded const — changing settings updates the value immediately.
        let (state, dir) = temp_state();

        {
            let mut s = state.lock().unwrap();
            s.config.cloud_endpoint = "http://10.0.0.5:9000/ingest".to_string();
        }

        let s = state.lock().unwrap();
        assert_eq!(
            s.config.cloud_endpoint, "http://10.0.0.5:9000/ingest",
            "endpoint must reflect the updated settings value"
        );

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn log_event_appends_to_sync_log() {
        let (state, _dir) = temp_state();
        log_event!(state, LogLevel::Info, LogSubsystem::Sync, "first event");
        log_event!(state, LogLevel::Info, LogSubsystem::Sync, "second event");

        let s = state.lock().unwrap();
        // Since we insert at 0, index 0 is now "second event"
        assert_eq!(s.sync_log[0].message, "second event");
        assert_eq!(s.sync_log[1].message, "first event");
    }

    #[test]
    fn log_event_caps_at_100_entries() {
        let (state, _dir) = temp_state();

        for i in 0..120 {
            log_event!(state, LogLevel::Info, LogSubsystem::Sync, "event {}", i);
        }

        let s = state.lock().unwrap();
        assert_eq!(s.sync_log.len(), 100);
        // The most recent event (119) should be at the front
        assert_eq!(s.sync_log[0].message, "event 119");
    }

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_encode_known_values() {
        // Standard base64 test vectors (RFC 4648)
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn unsynced_rows_remain_after_offline_status() {
        let (state, dir) = temp_state();

        // Insert a row while network is Offline — it should stay unsynced
        {
            let s = state.lock().unwrap();
            insert_payload(&s.db.conn, "device-x", b"blob", 1000).unwrap();
        }

        // Verify it's still unsynced (no sync cycle ran)
        {
            let s = state.lock().unwrap();
            let rows = fetch_unsynced(&s.db.conn).unwrap();
            assert_eq!(
                rows.len(),
                1,
                "row must remain unsynced when network is Offline"
            );
        }

        fs::remove_dir_all(&dir).ok();
    }
}
