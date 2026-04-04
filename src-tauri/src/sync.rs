// sync.rs
// Batch sync engine — drains unsynced rows to the cloud when network is Stable.

use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

use crate::db::queries::{fetch_unsynced, mark_synced};
use crate::state::{AppState, NetworkStatus, SyncEvent};

const SYNC_INTERVAL_SECS: u64 = 10;

/// The cloud endpoint that receives batched payloads.
/// TODO: replace with real cloud URL before deployment.
const CLOUD_ENDPOINT: &str = "http://127.0.0.1:9000/ingest";

/// Starts the sync engine loop.
///
/// Runs forever — spawn with `tokio::spawn` from `main.rs` at startup.
///
/// Every `SYNC_INTERVAL_SECS` seconds:
///   1. Checks if network is `Stable` — skips if not
///   2. Fetches all unsynced rows from the DB
///   3. Posts one batched request to the cloud endpoint
///   4. On success: marks all rows as synced
///   5. On failure: skips the batch, logs the error, rows retry next cycle
pub async fn start_sync(state: Arc<Mutex<AppState>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("Failed to build reqwest client for sync engine");

    let mut ticker = interval(Duration::from_secs(SYNC_INTERVAL_SECS));

    loop {
        ticker.tick().await;

        // --- Check network status without cloning the enum ---
        // Extract a bool so we can drop the lock before any I/O.
        let is_stable = match state.lock() {
            Ok(s) => s.network_status == NetworkStatus::Stable,
            Err(e) => {
                eprintln!("[sync] AppState lock poisoned: {e}; sync engine shutting down");
                return;
            }
        };

        if !is_stable {
            continue;
        }

        // --- Fetch unsynced rows (lock held only for the DB query) ---
        let rows = match state.lock() {
            Ok(s) => match fetch_unsynced(&s.db.conn) {
                Ok(r) => r,
                Err(e) => {
                    log_event(&state, format!("Failed to fetch unsynced rows: {e}"));
                    continue;
                }
            },
            Err(e) => {
                eprintln!("[sync] AppState lock poisoned: {e}; sync engine shutting down");
                return;
            }
        };

        if rows.is_empty() {
            continue;
        }

        println!("[sync] Syncing {} row(s) to cloud...", rows.len());

        // --- Build the batch payload (no lock needed) ---
        let batch: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "id":             row.id,
                    "device_id":      row.device_id,
                    "encrypted_blob": base64_encode(&row.encrypted_blob),
                    "received_at":    row.received_at,
                })
            })
            .collect();

        // --- POST the batch (no lock held during network I/O) ---
        let post_result = client
            .post(CLOUD_ENDPOINT)
            .json(&serde_json::json!({ "payloads": batch }))
            .send()
            .await;

        match post_result {
            Ok(response) if response.status().is_success() => {
                let mut synced_count = 0usize;
                let mut failed_ids: Vec<i64> = Vec::new();

                match state.lock() {
                    Ok(s) => {
                        for row in &rows {
                            match mark_synced(&s.db.conn, row.id) {
                                Ok(()) => synced_count += 1,
                                Err(e) => {
                                    eprintln!(
                                        "[sync] Failed to mark row {} as synced: {e}",
                                        row.id
                                    );
                                    failed_ids.push(row.id);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[sync] AppState lock poisoned after upload: {e}");
                        return;
                    }
                }

                let msg = if failed_ids.is_empty() {
                    format!("Synced {synced_count} row(s) successfully")
                } else {
                    format!(
                        "Synced {synced_count} row(s); failed to mark {} row(s): {failed_ids:?}",
                        failed_ids.len()
                    )
                };

                println!("[sync] {msg}");
                log_event(&state, msg);
            }

            Ok(response) => {
                let msg = format!(
                    "Batch upload rejected (HTTP {}); {} row(s) will retry",
                    response.status(),
                    rows.len()
                );
                eprintln!("[sync] {msg}");
                log_event(&state, msg);
            }

            Err(e) => {
                let msg = format!(
                    "Batch upload failed: {e}; {} row(s) will retry",
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
/// Caps the log at 100 entries. Uses VecDeque::pop_front() — O(1) — to drop
/// the oldest entry when the cap is exceeded (Vec::drain(0..n) was O(n)).
fn log_event(state: &Arc<Mutex<AppState>>, message: String) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut s = match state.lock() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[sync] Cannot write to sync log — lock poisoned: {e}");
            return;
        }
    };

    s.sync_log.push_back(SyncEvent { message, timestamp });

    // Drop oldest entries beyond the 100-entry cap.
    while s.sync_log.len() > 100 {
        s.sync_log.pop_front();
    }
}

/// Encodes bytes as a standard base64 string.
/// Implemented inline to avoid pulling in an extra crate.
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

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
        // VecDeque supports index access just like Vec
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
        // Oldest 20 entries (event 0..=19) must have been dropped
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
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn unsynced_rows_remain_after_offline_status() {
        let (state, dir) = temp_state();

        {
            let s = state.lock().unwrap();
            insert_payload(&s.db.conn, "device-x", b"blob", 1000).unwrap();
        }

        {
            let s = state.lock().unwrap();
            let rows = fetch_unsynced(&s.db.conn).unwrap();
            assert_eq!(rows.len(), 1, "row must remain unsynced when no sync ran");
        }

        fs::remove_dir_all(&dir).ok();
    }
}
