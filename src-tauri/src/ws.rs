// ws.rs
// WebSocket ingestion server — receives raw IoT payloads, encrypts them,
// and stores them in the local SQLCipher database.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::crypto::encrypt_payload;
use crate::db::queries::insert_payload;
use crate::error::SentinelError;
use crate::state::AppState;

/// The port the WebSocket ingestion server listens on.
pub const WS_PORT: u16 = 6767;

/// Bind address for the WebSocket server.
///
/// "0.0.0.0" binds to all interfaces simultaneously:
///   - 127.0.0.1 (loopback — same machine)
///   - 10.251.58.25 (LAN — laptop agent, phone agent)
///
/// This means no changes are needed when switching between local and
/// LAN devices — all interfaces are covered by a single bind.
const WS_BIND_ADDR: &str = "0.0.0.0";

/// Starts the WebSocket server and accepts incoming IoT device connections.
///
/// Each accepted connection is handed off to its own `tokio::spawn` task so
/// the server loop is never blocked by a slow or disconnected device.
///
/// Runs forever — spawn with `tokio::spawn` from `main.rs` at startup.
pub async fn start_server(state: Arc<Mutex<AppState>>) {
    let addr = format!("{WS_BIND_ADDR}:{WS_PORT}");

    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind WebSocket server on {addr}: {e}"));

    println!("[ws] Sentinel ingestion server listening on ws://{addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                println!("[ws] Device connected: {peer_addr}");
                let state = Arc::clone(&state);
                let peer = peer_addr.to_string();
                tokio::spawn(handle_connection(stream, state, peer));
            }
            Err(e) => {
                // A single failed accept must never kill the server loop.
                eprintln!("[ws] Accept error: {e}");
            }
        }
    }
}

/// Handles a single WebSocket connection for its full lifetime.
///
/// Protocol per message:
///   1. Extract encryption key (brief lock, key is `Copy` — no allocation)
///   2. Encrypt the raw bytes outside the lock (AES-256-GCM is CPU-bound)
///   3. Insert the encrypted blob into the DB (second, minimal lock)
///   4. Send a 1-byte ACK (0x01) to confirm delivery
///
/// The device tracking TODO is wired here:
///   - `connected_devices.insert` on successful handshake
///   - `connected_devices.remove` on disconnect / error
async fn handle_connection(stream: TcpStream, state: Arc<Mutex<AppState>>, peer_addr: String) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[ws] Handshake failed for {peer_addr}: {e}");
            return;
        }
    };

    // --- Register device on connect ---
    if let Ok(mut s) = state.lock() {
        s.connected_devices.insert(peer_addr.clone());
    }

    let (mut sender, mut receiver) = ws_stream.split();

    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(Message::Binary(payload)) => {
                if let Err(e) = process_payload(&state, &peer_addr, &payload) {
                    eprintln!("[ws] Failed to process payload from {peer_addr}: {e}");
                } else {
                    let _ = sender.send(Message::Binary(vec![0x01])).await;
                }
            }

            Ok(Message::Text(text)) => {
                // Accept text frames — treat UTF-8 bytes as raw payload.
                if let Err(e) = process_payload(&state, &peer_addr, text.as_bytes()) {
                    eprintln!("[ws] Failed to process text payload from {peer_addr}: {e}");
                } else {
                    let _ = sender.send(Message::Binary(vec![0x01])).await;
                }
            }

            Ok(Message::Close(_)) => {
                println!("[ws] Device disconnected: {peer_addr}");
                break;
            }

            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
            }

            Ok(_) => {} // Pong / Frame variants — ignore

            Err(e) => {
                eprintln!("[ws] Connection error from {peer_addr}: {e}");
                break;
            }
        }
    }

    // --- Deregister device on disconnect ---
    if let Ok(mut s) = state.lock() {
        s.connected_devices.remove(&peer_addr);
    }
}

/// Encrypts a raw payload and inserts it into the database.
///
/// ### Concurrency design
///
/// The previous version held the `AppState` lock across both the `encrypt`
/// call (CPU-bound) and the `insert_payload` call (I/O-bound DB write),
/// blocking every other thread for the full duration.
///
/// This version splits that into two minimal, non-overlapping lock windows:
///
/// 1. **Read lock** — copy the 32-byte key ([u8;32] is `Copy`, zero heap
///    allocation) then immediately release.
/// 2. **Encrypt** — runs entirely outside the lock.
/// 3. **Write lock** — hold only for the DB insert, then release.
///
/// This means concurrent WebSocket connections and Tauri commands are not
/// serialised behind crypto work.
fn process_payload(
    state: &Arc<Mutex<AppState>>,
    device_id: &str,
    raw: &[u8],
) -> Result<(), SentinelError> {
    // Step 1: copy the 32-byte key — lock held for one field copy only.
    let key = state.lock()?.encryption_key;

    // Step 2: encrypt outside the lock — no state access needed.
    let encrypted = encrypt_payload(raw, &key)?;

    // Step 3: timestamp — outside the lock.
    let received_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    // Step 4: DB insert — reacquire lock for the minimum needed window.
    let s = state.lock()?;
    insert_payload(&s.db.conn, device_id, &encrypted, received_at)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::decrypt_payload;
    use crate::db::{queries::fetch_unsynced, Db};
    use std::fs;

    fn temp_db() -> (Db, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "sentinel_ws_test_{:?}_{}",
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let db = Db::open(&dir).expect("Db::open must succeed");
        (db, dir)
    }

    #[test]
    fn process_payload_encrypts_and_stores() {
        let (db, dir) = temp_db();
        let state = Arc::new(Mutex::new(AppState::new(db)));

        let raw = b"temperature:42.5,humidity:60";
        process_payload(&state, "sensor-01", raw).expect("process_payload must succeed");

        let s = state.lock().unwrap();
        let rows = fetch_unsynced(&s.db.conn).expect("fetch must succeed");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].device_id, "sensor-01");

        let decrypted = decrypt_payload(&rows[0].encrypted_blob, &s.encryption_key)
            .expect("decryption must succeed");
        assert_eq!(decrypted, raw);

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn process_payload_multiple_devices() {
        let (db, dir) = temp_db();
        let state = Arc::new(Mutex::new(AppState::new(db)));

        process_payload(&state, "device-A", b"payload-A").unwrap();
        process_payload(&state, "device-B", b"payload-B").unwrap();
        process_payload(&state, "device-A", b"payload-C").unwrap();

        let s = state.lock().unwrap();
        let rows = fetch_unsynced(&s.db.conn).unwrap();
        assert_eq!(rows.len(), 3);

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }
}
