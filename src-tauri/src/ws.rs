use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::crypto::encrypt_payload;
use crate::db::queries::insert_payload;
use crate::state::AppState;

/// Starts the WebSocket server and listens for incoming IoT device connections.
///
/// Each accepted connection is handed off to its own `tokio::spawn` task so
/// the server loop never blocks — a slow or disconnected device cannot stall
/// other devices.
///
/// Bind address is read from `state.settings.ws_bind_address` at startup.
/// To change it, update settings and restart the app (rebinding a live TCP
/// listener requires stopping the task — runtime rebind is not supported).
///
/// This function runs forever and should be spawned with `tokio::spawn` from
/// `main.rs` during app startup.
pub async fn start_server(state: Arc<Mutex<AppState>>) {
    // Read bind address from settings — short lock, clone, release immediately.
    let addr = {
        let s = state.lock().expect("Lock poisoned");
        // Use the new config field name 'ws_port' instead of 'ws_bind_address'
        format!("0.0.0.0:{}", s.config.ws_port)
    };

    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {}", addr));

    println!("[ws] Sentinel ingestion server listening on ws://{addr}");

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
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
/// Lifecycle:
///   - On successful handshake: `peer_addr` is pushed to `connected_devices`
///   - On any exit (close, error, stream end): `peer_addr` is removed so the
///     dashboard always reflects live state. Early returns before registration
///     (handshake failure) skip the cleanup entirely.
///
/// For every incoming binary or text message:
///   1. Encrypts the raw bytes with AES-256-GCM via `crypto::encrypt_payload`
///   2. Inserts the encrypted blob into the DB via `db::queries::insert_payload`
///   3. Sends back a lightweight `ACK` so the device knows the payload landed
async fn handle_connection(stream: TcpStream, state: Arc<Mutex<AppState>>, peer_addr: String) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[ws] Handshake failed for {peer_addr}: {e}");
            return; // Never registered — no cleanup needed
        }
    };

    // --- Register device as connected ---
    {
        let mut s = state
            .lock()
            .expect("AppState lock poisoned on device connect");
        s.connected_devices.push(peer_addr.clone());
        println!(
            "[ws] Device connected: {peer_addr} ({} total)",
            s.connected_devices.len()
        );
    }

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
                println!("[ws] Device closed connection: {peer_addr}");
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

    // --- Deregister device — runs on ALL exit paths after registration ---
    {
        let mut s = state
            .lock()
            .expect("AppState lock poisoned on device disconnect");
        s.connected_devices.retain(|d| d != &peer_addr);
        println!(
            "[ws] Device removed: {peer_addr} ({} remaining)",
            s.connected_devices.len()
        );
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
) -> Result<(), crate::error::SentinelError> {
    let s = state.lock()?;

    let encrypted = encrypt_payload(raw, &s.encryption_key)?;

    let received_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
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
    fn connected_devices_push_and_retain() {
        // Simulates what handle_connection does on connect and disconnect.
        // Tests the state management pattern directly without a real socket.
        let (db, dir) = temp_db();
        let state = Arc::new(Mutex::new(AppState::new(
            db,
            crate::config::Config::default(),
            std::path::PathBuf::from("test_data"),
        )));

        let peer_a = "192.168.1.10:54321".to_string();
        let peer_b = "192.168.1.11:54322".to_string();

        // Simulate two devices connecting
        {
            let mut s = state.lock().unwrap();
            s.connected_devices.push(peer_a.clone());
            s.connected_devices.push(peer_b.clone());
        }

        {
            let s = state.lock().unwrap();
            assert_eq!(s.connected_devices.len(), 2);
            assert!(s.connected_devices.contains(&peer_a));
            assert!(s.connected_devices.contains(&peer_b));
        }

        // Simulate peer_a disconnecting
        {
            let mut s = state.lock().unwrap();
            s.connected_devices.retain(|d| d != &peer_a);
        }

        {
            let s = state.lock().unwrap();
            assert_eq!(s.connected_devices.len(), 1);
            assert!(
                !s.connected_devices.contains(&peer_a),
                "peer_a must be removed"
            );
            assert!(s.connected_devices.contains(&peer_b), "peer_b must remain");
        }

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn connected_devices_empty_after_all_disconnect() {
        let (db, dir) = temp_db();
        let state = Arc::new(Mutex::new(AppState::new(
            db,
            crate::config::Config::default(),
            std::path::PathBuf::from("test_data"),
        )));

        let peers = vec![
            "10.0.0.1:1001".to_string(),
            "10.0.0.2:1002".to_string(),
            "10.0.0.3:1003".to_string(),
        ];

        // All connect
        {
            let mut s = state.lock().unwrap();
            for p in &peers {
                s.connected_devices.push(p.clone());
            }
        }

        // All disconnect one by one
        for peer in &peers {
            let mut s = state.lock().unwrap();
            s.connected_devices.retain(|d| d != peer);
        }

        let s = state.lock().unwrap();
        assert!(
            s.connected_devices.is_empty(),
            "connected_devices must be empty after all peers disconnect"
        );

        drop(s);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn process_payload_encrypts_and_stores() {
        let (db, dir) = temp_db();
        let state = Arc::new(Mutex::new(AppState::new(
            db,
            crate::config::Config::default(),
            std::path::PathBuf::from("test_data"),
        )));

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
        let state = Arc::new(Mutex::new(AppState::new(
            db,
            crate::config::Config::default(),
            std::path::PathBuf::from("test_data"),
        )));

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
