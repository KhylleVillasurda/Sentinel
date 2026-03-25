use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::crypto::encrypt_payload;
use crate::db::queries::insert_payload;
use crate::state::AppState;

/// The port the WebSocket ingestion server listens on.
/// IoT devices connect to ws://localhost:6767
pub const WS_PORT: u16 = 6767;

/// Starts the WebSocket server and listens for incoming IoT device connections.
///
/// Each accepted connection is handed off to its own `tokio::spawn` task so
/// the server loop never blocks — a slow or disconnected device cannot stall
/// other devices.
///
/// This function runs forever and should be spawned with `tokio::spawn` from
/// `main.rs` during app startup.
pub async fn start_server(state: Arc<Mutex<AppState>>) {
    let addr = format!("127.0.0.1:{WS_PORT}");

    let listener = TcpListener::bind(&addr)
        .await
        .expect(&format!("Failed to bind WebSocket server on {addr}"));

    println!("[ws] Sentinel ingestion server listening on ws://{addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                println!("[ws] Device connected: {peer_addr}");
                let state = Arc::clone(&state);
                tokio::spawn(handle_connection(stream, state, peer_addr.to_string()));
            }
            Err(e) => {
                // Log and continue — a single failed accept should never kill
                // the server loop
                eprintln!("[ws] Accept error: {e}");
            }
        }
    }
}

/// Handles a single WebSocket connection for its full lifetime.
///
/// For every incoming binary or text message:
///   1. Encrypts the raw bytes with AES-256-GCM via `crypto::encrypt_payload`
///   2. Inserts the encrypted blob into the DB via `db::queries::insert_payload`
///   3. Sends back a lightweight `ACK` so the device knows the payload landed
///
/// On disconnect or error the task exits cleanly — the server loop is unaffected.
async fn handle_connection(stream: TcpStream, state: Arc<Mutex<AppState>>, peer_addr: String) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[ws] Handshake failed for {peer_addr}: {e}");
            return;
        }
    };

    let (mut sender, mut receiver) = ws_stream.split();

    while let Some(msg_result) = receiver.next().await {
        match msg_result {
            Ok(Message::Binary(payload)) => {
                if let Err(e) = process_payload(&state, &peer_addr, &payload) {
                    eprintln!("[ws] Failed to process payload from {peer_addr}: {e}");
                } else {
                    // ACK: single byte 0x01 so devices can confirm delivery
                    let _ = sender.send(Message::Binary(vec![0x01])).await;
                }
            }

            Ok(Message::Text(text)) => {
                // Accept text frames too — treat UTF-8 bytes as raw payload
                let payload = text.into_bytes();
                if let Err(e) = process_payload(&state, &peer_addr, &payload) {
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
                // Respond to pings to keep connections alive
                let _ = sender.send(Message::Pong(data)).await;
            }

            Ok(_) => {} // Pong / Frame variants — ignore

            Err(e) => {
                eprintln!("[ws] Connection error from {peer_addr}: {e}");
                break;
            }
        }
    }
}

/// Encrypts a raw payload and inserts it into the database.
///
/// Extracted from `handle_connection` so it can be called for both
/// binary and text frames without duplication.
fn process_payload(
    state: &Arc<Mutex<AppState>>,
    device_id: &str,
    raw: &[u8],
) -> Result<(), String> {
    let s = state
        .lock()
        .map_err(|e| format!("State lock poisoned: {e}"))?;

    let encrypted = encrypt_payload(raw, &s.encryption_key)?;

    let received_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("System time error: {e}"))?
        .as_secs() as i64;

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

        // Fetch from DB and verify the blob decrypts back to the original
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
