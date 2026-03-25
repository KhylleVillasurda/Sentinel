use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use crate::db::queries::{fetch_unsynced, mark_synced};
use crate::state::{AppState, NetworkStatus, SyncEvent};

/// How often the sync engine checks for unsynced rows when network is Stable.
const SYNC_INTERVAL_SECS: u64 = 10;

/// The cloud endpoint that receives batched payloads.
/// TODO (Phase 5 / deployment): move this to tauri.conf.json or a config file.
const CLOUD_ENDPOINT: &str = "https://your-cloud-endpoint.example.com/ingest";

/// Starts the sync engine loop.
///
/// Runs forever — spawn with `tokio::spawn` from `main.rs` at startup.
/// Every `SYNC_INTERVAL_SECS` seconds:
///   1. Checks if network is `Stable` — skips the cycle if not
///   2. Fetches all unsynced rows from the DB
///   3. Attempts a single batched POST to the cloud endpoint
///   4. On success: marks all rows as synced
///   5. On failure: skips the failed batch, logs the error, and continues
pub async fn start_sync(state: Arc<Mutex<AppState>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build reqwest client for sync engine");

    let mut ticker = interval(Duration::from_secs(SYNC_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        // --- Check network status before doing anything ---
        let network_status = {
            let s = state.lock().expect("AppState lock poisoned in sync engine");
            s.network_status.clone()
        };

        if network_status != NetworkStatus::Stable {
            continue; // Not stable — skip this cycle silently
        }

        // --- Fetch unsynced rows ---
        let rows = {
            let s = state.lock().expect("AppState lock poisoned in sync engine");
            match fetch_unsynced(&s.db.conn) {
                Ok(r) => r,
                Err(e) => {
                    log_event(&state, format!("Failed to fetch unsynced rows: {e}"));
                    continue;
                }
            }
        };

        if rows.is_empty() {
            continue; // Nothing to sync — skip silently
        }

        println!("[sync] Syncing {} row(s) to cloud...", rows.len());

        // --- Build the batch payload ---
        // Each row is serialized as a JSON object with its id and base64-encoded blob.
        // The cloud endpoint receives an array of these objects.
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

        // --- POST the batch ---
        let post_result = client
            .post(CLOUD_ENDPOINT)
            .json(&serde_json::json!({ "payloads": batch }))
            .send()
            .await;

        match post_result {
            Ok(response) if response.status().is_success() => {
                // --- Mark all rows as synced ---
                let mut synced_count = 0;
                let mut failed_ids: Vec<i64> = Vec::new();

                {
                    let s = state.lock().expect("AppState lock poisoned");
                    for row in &rows {
                        match mark_synced(&s.db.conn, row.id) {
                            Ok(_) => synced_count += 1,
                            Err(e) => {
                                eprintln!("[sync] Failed to mark row {} as synced: {e}", row.id);
                                failed_ids.push(row.id);
                            }
                        }
                    }
                }

                let msg = if failed_ids.is_empty() {
                    format!("Synced {synced_count} row(s) successfully")
                } else {
                    format!(
                        "Synced {synced_count} row(s); failed to mark {} row(s): {:?}",
                        failed_ids.len(),
                        failed_ids
                    )
                };

                println!("[sync] {msg}");
                log_event(&state, msg);
            }

            Ok(response) => {
                // Server responded but with an error status — skip and log
                let status = response.status();
                let msg = format!(
                    "Batch upload rejected by server (HTTP {status}); {} row(s) will retry next cycle",
                    rows.len()
                );
                eprintln!("[sync] {msg}");
                log_event(&state, msg);
            }

            Err(e) => {
                // Network error — skip and log, rows stay unsynced for next cycle
                let msg = format!(
                    "Batch upload failed: {e}; {} row(s) will retry next cycle",
                    rows.len()
                );
                eprintln!("[sync] {msg}");
                log_event(&state, msg);
            }
        }
    }
}

/// Appends a message to the rolling sync event log in `AppState`.
///
/// Caps the log at 100 entries — oldest entries are dropped first.
/// The dashboard reads this log to display recent sync activity.
fn log_event(state: &Arc<Mutex<AppState>>, message: String) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut s = state.lock().expect("AppState lock poisoned in log_event");
    s.sync_log.push(SyncEvent { message, timestamp });

    // Keep the log bounded — drop oldest entries beyond 100
    if s.sync_log.len() > 100 {
        let drain_count = s.sync_log.len() - 100;
        s.sync_log.drain(0..drain_count);
    }
}

/// Encodes bytes as a base64 string without pulling in an extra crate.
/// Uses the standard alphabet (A-Z, a-z, 0-9, +, /).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };

        out.push(ALPHABET[b0 >> 2] as char);
        out.push(ALPHABET[((b0 & 0x3) << 4) | (b1 >> 4)] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((b1 & 0xf) << 2) | (b2 >> 6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[b2 & 0x3f] as char
        } else {
            '='
        });
    }

    out
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
        let state = Arc::new(Mutex::new(AppState::new(db)));
        (state, dir)
    }

    #[test]
    fn log_event_appends_to_sync_log() {
        let (state, dir) = temp_state();

        log_event(&state, "first event".to_string());
        log_event(&state, "second event".to_string());

        let s = state.lock().unwrap();
        assert_eq!(s.sync_log.len(), 2);
        assert_eq!(s.sync_log[0].message, "first event");
        assert_eq!(s.sync_log[1].message, "second event");

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn log_event_caps_at_100_entries() {
        let (state, dir) = temp_state();

        for i in 0..120 {
            log_event(&state, format!("event {i}"));
        }

        let s = state.lock().unwrap();
        assert_eq!(s.sync_log.len(), 100);
        // Oldest entries should have been dropped — first entry is now "event 20"
        assert_eq!(s.sync_log[0].message, "event 20");

        drop(s);
        fs::remove_dir_all(&dir).ok();
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
